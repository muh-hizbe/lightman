// Storage layer — SQLite-backed persistence via rusqlite.
// All DB operations run via tokio::task::spawn_blocking to keep the async executor unblocked.
//
// Task 2.1: Database initialization and migrations.

use rusqlite::{Connection, Result as SqlResult, params};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// A thread-safe handle to the SQLite database connection.
pub type DbHandle = Arc<Mutex<Connection>>;

/// Opens (or creates) the SQLite database at `db_path`, enables WAL mode,
/// runs all pending schema migrations, and returns a thread-safe handle.
pub fn initialize_database(db_path: PathBuf) -> SqlResult<DbHandle> {
    let conn = Connection::open(&db_path)?;

    // Enable WAL mode for better concurrent read performance and crash safety.
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;

    // Enable foreign key enforcement.
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;

    // Run migrations.
    run_migrations(&conn)?;

    Ok(Arc::new(Mutex::new(conn)))
}

/// Runs all schema migrations in version order.
/// Each migration is applied exactly once; already-applied versions are skipped.
///
/// Exposed as `pub(crate)` so that unit tests in other modules can set up an
/// in-memory database without going through the full `initialize_database` path.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn run_migrations_for_test(conn: &Connection) -> SqlResult<()> {
    run_migrations(conn)
}

fn run_migrations(conn: &Connection) -> SqlResult<()> {
    // Ensure the migration tracking table exists first.
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version     INTEGER PRIMARY KEY,
            applied_at  TEXT NOT NULL
        );",
    )?;

    // Apply each migration if it has not been applied yet.
    apply_migration(conn, 1, MIGRATION_V1)?;
    apply_migration(conn, 2, MIGRATION_V2)?;
    apply_migration(conn, 3, MIGRATION_V3)?;
    apply_migration(conn, 4, MIGRATION_V4)?;

    Ok(())
}

/// Applies a single migration identified by `version` if it has not been recorded yet.
fn apply_migration(conn: &Connection, version: i64, sql: &str) -> SqlResult<()> {
    let already_applied: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM schema_migrations WHERE version = ?1",
            params![version],
            |row| row.get::<_, i64>(0),
        )
        .map(|count| count > 0)
        .unwrap_or(false);

    if already_applied {
        return Ok(());
    }

    // Execute the migration DDL.
    conn.execute_batch(sql)?;

    // Record the migration as applied.
    let applied_at = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
        params![version, applied_at],
    )?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Migration v1 — full initial schema
// ---------------------------------------------------------------------------

const MIGRATION_V1: &str = "
-- Collections and folders (self-referential for nested folders)
CREATE TABLE IF NOT EXISTS collections (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL CHECK(length(name) BETWEEN 1 AND 255),
    description TEXT,
    parent_id   TEXT REFERENCES collections(id) ON DELETE CASCADE,
    position    INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

-- HTTP request configurations
CREATE TABLE IF NOT EXISTS requests (
    id              TEXT PRIMARY KEY,
    collection_id   TEXT NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    name            TEXT NOT NULL CHECK(length(name) BETWEEN 1 AND 255),
    method          TEXT NOT NULL,
    url             TEXT NOT NULL,
    headers_json    TEXT NOT NULL DEFAULT '[]',
    params_json     TEXT NOT NULL DEFAULT '[]',
    body_json       TEXT NOT NULL DEFAULT '{\"type\":\"none\"}',
    auth_json       TEXT NOT NULL DEFAULT '{\"type\":\"none\"}',
    position        INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

-- Environments
CREATE TABLE IF NOT EXISTS environments (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL CHECK(length(name) BETWEEN 1 AND 255),
    is_active   INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

-- Variables (environment-scoped, or global when environment_id IS NULL)
CREATE TABLE IF NOT EXISTS variables (
    id              TEXT PRIMARY KEY,
    environment_id  TEXT REFERENCES environments(id) ON DELETE CASCADE,
    key             TEXT NOT NULL,
    value           TEXT NOT NULL DEFAULT '',
    enabled         INTEGER NOT NULL DEFAULT 1
);

-- Request execution history
CREATE TABLE IF NOT EXISTS history (
    id                    TEXT PRIMARY KEY,
    request_json          TEXT NOT NULL,
    status_code           INTEGER NOT NULL,
    response_time_ms      INTEGER NOT NULL,
    response_size_bytes   INTEGER NOT NULL,
    response_headers_json TEXT NOT NULL DEFAULT '[]',
    response_body         TEXT NOT NULL DEFAULT '',
    executed_at           TEXT NOT NULL
);

-- Indexes for common query patterns
CREATE INDEX IF NOT EXISTS idx_requests_collection ON requests(collection_id);
CREATE INDEX IF NOT EXISTS idx_variables_env       ON variables(environment_id);
CREATE INDEX IF NOT EXISTS idx_history_executed    ON history(executed_at DESC);
";

// ---------------------------------------------------------------------------
// Migration v2 — add auth_json to collections
// ---------------------------------------------------------------------------

const MIGRATION_V2: &str = "
ALTER TABLE collections ADD COLUMN auth_json TEXT NOT NULL DEFAULT '{\"type\":\"none\"}';
";

// ---------------------------------------------------------------------------
// Migration v3 — saved_responses table
// ---------------------------------------------------------------------------

const MIGRATION_V3: &str = "
CREATE TABLE IF NOT EXISTS saved_responses (
    id                    TEXT PRIMARY KEY,
    request_id            TEXT NOT NULL REFERENCES requests(id) ON DELETE CASCADE,
    label                 TEXT NOT NULL DEFAULT 'Response',
    status_code           INTEGER NOT NULL,
    status_text           TEXT NOT NULL DEFAULT '',
    response_headers_json TEXT NOT NULL DEFAULT '[]',
    response_body         TEXT NOT NULL DEFAULT '',
    response_time_ms      INTEGER NOT NULL,
    response_size_bytes   INTEGER NOT NULL,
    saved_at              TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_saved_responses_request ON saved_responses(request_id);
";

// ---------------------------------------------------------------------------
// Migration v4 — add status_text to history table
// ---------------------------------------------------------------------------

const MIGRATION_V4: &str = "
ALTER TABLE history ADD COLUMN status_text TEXT NOT NULL DEFAULT '';
";

// ---------------------------------------------------------------------------
// Task 2.2 — CRUD for Collections and Requests
// All public functions accept a `DbHandle` and wrap the blocking rusqlite
// call inside `tokio::task::spawn_blocking` so the async executor is never
// blocked.
// ---------------------------------------------------------------------------

// ---- helpers ---------------------------------------------------------------

/// Map a `rusqlite::Error` to a human-readable `String`.
fn db_err(e: rusqlite::Error) -> String {
    e.to_string()
}

// ---- Collection CRUD -------------------------------------------------------

/// Insert a new collection row.
/// `id`, `created_at`, and `updated_at` must be pre-generated by the caller.
pub async fn insert_collection(
    handle: DbHandle,
    id: String,
    name: String,
    description: Option<String>,
    parent_id: Option<String>,
    position: i64,
    auth_json: String,
    created_at: String,
    updated_at: String,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let conn = handle.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO collections (id, name, description, parent_id, position, auth_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![id, name, description, parent_id, position, auth_json, created_at, updated_at],
        )
        .map_err(db_err)?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Fetch a single collection by its UUID string.
/// Returns `None` if not found.
pub async fn get_collection(
    handle: DbHandle,
    id: String,
) -> Result<Option<CollectionRow>, String> {
    tokio::task::spawn_blocking(move || {
        let conn = handle.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id, name, description, parent_id, position, auth_json, created_at, updated_at
                 FROM collections WHERE id = ?1",
            )
            .map_err(db_err)?;

        let mut rows = stmt
            .query_map(params![id], CollectionRow::from_row)
            .map_err(db_err)?;

        match rows.next() {
            Some(row) => Ok(Some(row.map_err(db_err)?)),
            None => Ok(None),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Return all collections ordered by `parent_id NULLS FIRST, position`.
pub async fn list_collections(handle: DbHandle) -> Result<Vec<CollectionRow>, String> {
    tokio::task::spawn_blocking(move || {
        let conn = handle.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id, name, description, parent_id, position, auth_json, created_at, updated_at
                 FROM collections
                 ORDER BY parent_id NULLS FIRST, position ASC",
            )
            .map_err(db_err)?;

        let rows = stmt
            .query_map([], CollectionRow::from_row)
            .map_err(db_err)?;

        rows.map(|r| r.map_err(db_err)).collect()
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Update name, description, auth_json, and updated_at for an existing collection.
pub async fn update_collection(
    handle: DbHandle,
    id: String,
    name: String,
    description: Option<String>,
    auth_json: String,
    updated_at: String,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let conn = handle.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE collections SET name = ?1, description = ?2, auth_json = ?3, updated_at = ?4 WHERE id = ?5",
            params![name, description, auth_json, updated_at, id],
        )
        .map_err(db_err)?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Delete a collection and all its descendants (cascade via FK ON DELETE CASCADE).
pub async fn delete_collection(handle: DbHandle, id: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let conn = handle.lock().map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM collections WHERE id = ?1", params![id])
            .map_err(db_err)?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---- Request CRUD ----------------------------------------------------------

/// Insert a new request row.
pub async fn insert_request(
    handle: DbHandle,
    id: String,
    collection_id: String,
    name: String,
    method: String,
    url: String,
    headers_json: String,
    params_json: String,
    body_json: String,
    auth_json: String,
    position: i64,
    created_at: String,
    updated_at: String,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let conn = handle.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO requests
             (id, collection_id, name, method, url, headers_json, params_json,
              body_json, auth_json, position, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                id, collection_id, name, method, url,
                headers_json, params_json, body_json, auth_json,
                position, created_at, updated_at
            ],
        )
        .map_err(db_err)?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Fetch a single request by its UUID string.
/// Returns `None` if not found.
pub async fn get_request(
    handle: DbHandle,
    id: String,
) -> Result<Option<RequestRow>, String> {
    tokio::task::spawn_blocking(move || {
        let conn = handle.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id, collection_id, name, method, url,
                        headers_json, params_json, body_json, auth_json,
                        position, created_at, updated_at
                 FROM requests WHERE id = ?1",
            )
            .map_err(db_err)?;

        let mut rows = stmt
            .query_map(params![id], RequestRow::from_row)
            .map_err(db_err)?;

        match rows.next() {
            Some(row) => Ok(Some(row.map_err(db_err)?)),
            None => Ok(None),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Return all requests belonging to a collection, ordered by position.
pub async fn list_requests_by_collection(
    handle: DbHandle,
    collection_id: String,
) -> Result<Vec<RequestRow>, String> {
    tokio::task::spawn_blocking(move || {
        let conn = handle.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id, collection_id, name, method, url,
                        headers_json, params_json, body_json, auth_json,
                        position, created_at, updated_at
                 FROM requests
                 WHERE collection_id = ?1
                 ORDER BY position ASC",
            )
            .map_err(db_err)?;

        let rows = stmt
            .query_map(params![collection_id], RequestRow::from_row)
            .map_err(db_err)?;

        rows.map(|r| r.map_err(db_err)).collect()
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Update the mutable fields of an existing request.
pub async fn update_request(
    handle: DbHandle,
    id: String,
    name: String,
    method: String,
    url: String,
    headers_json: String,
    params_json: String,
    body_json: String,
    auth_json: String,
    position: i64,
    updated_at: String,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let conn = handle.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE requests
             SET name = ?1, method = ?2, url = ?3,
                 headers_json = ?4, params_json = ?5,
                 body_json = ?6, auth_json = ?7,
                 position = ?8, updated_at = ?9
             WHERE id = ?10",
            params![
                name, method, url,
                headers_json, params_json, body_json, auth_json,
                position, updated_at, id
            ],
        )
        .map_err(db_err)?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Delete a single request by ID.
pub async fn delete_request(handle: DbHandle, id: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let conn = handle.lock().map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM requests WHERE id = ?1", params![id])
            .map_err(db_err)?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Move a request to a different collection/folder.
///
/// Updates `collection_id` to `target_collection_id` and sets `position` to
/// the end of the target collection (i.e. `COUNT(*) in target collection`).
pub async fn move_request(
    handle: DbHandle,
    id: String,
    target_collection_id: String,
    updated_at: String,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let conn = handle.lock().map_err(|e| e.to_string())?;
        // Count existing requests in target collection to compute new position.
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM requests WHERE collection_id = ?1",
                params![target_collection_id],
                |row| row.get(0),
            )
            .map_err(db_err)?;
        conn.execute(
            "UPDATE requests
             SET collection_id = ?1, position = ?2, updated_at = ?3
             WHERE id = ?4",
            params![target_collection_id, count, updated_at, id],
        )
        .map_err(db_err)?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------------------
// Task 2.3 — CRUD for Environments, Variables, and History
// ---------------------------------------------------------------------------

// ---- Environment CRUD ------------------------------------------------------

/// Insert a new environment row.
pub async fn insert_environment(
    handle: DbHandle,
    id: String,
    name: String,
    is_active: bool,
    created_at: String,
    updated_at: String,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let conn = handle.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO environments (id, name, is_active, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, name, is_active as i64, created_at, updated_at],
        )
        .map_err(db_err)?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Fetch the currently active environment.
/// Returns `None` if no environment is set as active.
pub async fn get_active_environment(handle: DbHandle) -> Result<Option<EnvironmentRow>, String> {
    tokio::task::spawn_blocking(move || {
        let conn = handle.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id, name, is_active, created_at, updated_at
                 FROM environments WHERE is_active = 1 LIMIT 1",
            )
            .map_err(db_err)?;

        let mut rows = stmt
            .query_map([], EnvironmentRow::from_row)
            .map_err(db_err)?;

        match rows.next() {
            Some(row) => Ok(Some(row.map_err(db_err)?)),
            None => Ok(None),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Return all environments ordered by name.
pub async fn list_environments(handle: DbHandle) -> Result<Vec<EnvironmentRow>, String> {
    tokio::task::spawn_blocking(move || {
        let conn = handle.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id, name, is_active, created_at, updated_at
                 FROM environments ORDER BY name ASC",
            )
            .map_err(db_err)?;

        let rows = stmt
            .query_map([], EnvironmentRow::from_row)
            .map_err(db_err)?;

        rows.map(|r| r.map_err(db_err)).collect()
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Update the name and is_active flag of an existing environment.
pub async fn update_environment(
    handle: DbHandle,
    id: String,
    name: String,
    is_active: bool,
    updated_at: String,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let conn = handle.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE environments SET name = ?1, is_active = ?2, updated_at = ?3 WHERE id = ?4",
            params![name, is_active as i64, updated_at, id],
        )
        .map_err(db_err)?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Delete an environment and all its variables (cascade via FK ON DELETE CASCADE).
pub async fn delete_environment(handle: DbHandle, id: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let conn = handle.lock().map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM environments WHERE id = ?1", params![id])
            .map_err(db_err)?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---- Variable CRUD ---------------------------------------------------------

/// Insert a new variable row.
/// Set `environment_id` to `None` for global variables.
pub async fn insert_variable(
    handle: DbHandle,
    id: String,
    environment_id: Option<String>,
    key: String,
    value: String,
    enabled: bool,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let conn = handle.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO variables (id, environment_id, key, value, enabled)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, environment_id, key, value, enabled as i64],
        )
        .map_err(db_err)?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Return all variables belonging to a specific environment, ordered by key.
pub async fn list_variables_by_env(
    handle: DbHandle,
    environment_id: String,
) -> Result<Vec<VariableRow>, String> {
    tokio::task::spawn_blocking(move || {
        let conn = handle.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id, environment_id, key, value, enabled
                 FROM variables WHERE environment_id = ?1 ORDER BY key ASC",
            )
            .map_err(db_err)?;

        let rows = stmt
            .query_map(params![environment_id], VariableRow::from_row)
            .map_err(db_err)?;

        rows.map(|r| r.map_err(db_err)).collect()
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Delete all variables belonging to a specific environment.
pub async fn delete_variables_by_env(
    handle: DbHandle,
    environment_id: String,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let conn = handle.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "DELETE FROM variables WHERE environment_id = ?1",
            params![environment_id],
        )
        .map_err(db_err)?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---- History CRUD ----------------------------------------------------------

/// Insert a new history entry row.
pub async fn insert_history_entry(
    handle: DbHandle,
    id: String,
    request_json: String,
    status_code: i64,
    status_text: String,
    response_time_ms: i64,
    response_size_bytes: i64,
    response_headers_json: String,
    response_body: String,
    executed_at: String,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let conn = handle.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO history
             (id, request_json, status_code, status_text, response_time_ms, response_size_bytes,
              response_headers_json, response_body, executed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                id,
                request_json,
                status_code,
                status_text,
                response_time_ms,
                response_size_bytes,
                response_headers_json,
                response_body,
                executed_at
            ],
        )
        .map_err(db_err)?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Return history entries ordered by `executed_at DESC` (most recent first).
/// Optionally limit the number of results with `limit`.
pub async fn list_history(
    handle: DbHandle,
    limit: Option<i64>,
) -> Result<Vec<HistoryRow>, String> {
    tokio::task::spawn_blocking(move || {
        let conn = handle.lock().map_err(|e| e.to_string())?;
        let sql = match limit {
            Some(n) => format!(
                "SELECT id, request_json, status_code, status_text, response_time_ms, response_size_bytes,
                        response_headers_json, response_body, executed_at
                 FROM history ORDER BY executed_at DESC LIMIT {n}"
            ),
            None => "SELECT id, request_json, status_code, status_text, response_time_ms, response_size_bytes,
                            response_headers_json, response_body, executed_at
                     FROM history ORDER BY executed_at DESC"
                .to_string(),
        };

        let mut stmt = conn.prepare(&sql).map_err(db_err)?;
        let rows = stmt
            .query_map([], HistoryRow::from_row)
            .map_err(db_err)?;

        rows.map(|r| r.map_err(db_err)).collect()
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Delete a single history entry by ID.
pub async fn delete_history_entry(handle: DbHandle, id: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let conn = handle.lock().map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM history WHERE id = ?1", params![id])
            .map_err(db_err)?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Delete all history entries.
pub async fn clear_history(handle: DbHandle) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let conn = handle.lock().map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM history", []).map_err(db_err)?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------------------
// Task 8.1 — Extra Variable helpers needed by EnvironmentService
// ---------------------------------------------------------------------------

/// Fetch a single variable by its UUID.
/// Returns `None` if not found.
pub async fn get_variable_by_id(
    handle: DbHandle,
    id: String,
) -> Result<Option<VariableRow>, String> {
    tokio::task::spawn_blocking(move || {
        let conn = handle.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id, environment_id, key, value, enabled
                 FROM variables WHERE id = ?1",
            )
            .map_err(db_err)?;

        let mut rows = stmt
            .query_map(params![id], VariableRow::from_row)
            .map_err(db_err)?;

        match rows.next() {
            Some(row) => Ok(Some(row.map_err(db_err)?)),
            None => Ok(None),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Return all global variables (those where `environment_id IS NULL`), ordered by key.
pub async fn list_global_variables(handle: DbHandle) -> Result<Vec<VariableRow>, String> {
    tokio::task::spawn_blocking(move || {
        let conn = handle.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id, environment_id, key, value, enabled
                 FROM variables WHERE environment_id IS NULL ORDER BY key ASC",
            )
            .map_err(db_err)?;

        let rows = stmt
            .query_map([], VariableRow::from_row)
            .map_err(db_err)?;

        rows.map(|r| r.map_err(db_err)).collect()
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Update the key, value, and enabled flag of an existing variable.
pub async fn update_variable(
    handle: DbHandle,
    id: String,
    key: String,
    value: String,
    enabled: bool,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let conn = handle.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE variables SET key = ?1, value = ?2, enabled = ?3 WHERE id = ?4",
            params![key, value, enabled as i64, id],
        )
        .map_err(db_err)?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Delete a single variable by its UUID.
pub async fn delete_variable(handle: DbHandle, id: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let conn = handle.lock().map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM variables WHERE id = ?1", params![id])
            .map_err(db_err)?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---- Row types -------------------------------------------------------------
// Plain data structs that mirror exactly one row from the DB.
// The services layer is responsible for converting these into domain models.

/// Raw row from the `collections` table.
#[derive(Debug, Clone)]
pub struct CollectionRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub parent_id: Option<String>,
    pub position: i64,
    pub auth_json: String,
    pub created_at: String,
    pub updated_at: String,
}

impl CollectionRow {
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            name: row.get(1)?,
            description: row.get(2)?,
            parent_id: row.get(3)?,
            position: row.get(4)?,
            auth_json: row.get(5)?,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
        })
    }
}

/// Raw row from the `requests` table.
#[derive(Debug, Clone)]
pub struct RequestRow {
    pub id: String,
    pub collection_id: String,
    pub name: String,
    pub method: String,
    pub url: String,
    pub headers_json: String,
    pub params_json: String,
    pub body_json: String,
    pub auth_json: String,
    pub position: i64,
    pub created_at: String,
    pub updated_at: String,
}

impl RequestRow {
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            collection_id: row.get(1)?,
            name: row.get(2)?,
            method: row.get(3)?,
            url: row.get(4)?,
            headers_json: row.get(5)?,
            params_json: row.get(6)?,
            body_json: row.get(7)?,
            auth_json: row.get(8)?,
            position: row.get(9)?,
            created_at: row.get(10)?,
            updated_at: row.get(11)?,
        })
    }
}

/// Raw row from the `environments` table.
#[derive(Debug, Clone)]
pub struct EnvironmentRow {
    pub id: String,
    pub name: String,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl EnvironmentRow {
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            name: row.get(1)?,
            is_active: row.get::<_, i64>(2)? != 0,
            created_at: row.get(3)?,
            updated_at: row.get(4)?,
        })
    }
}

/// Raw row from the `variables` table.
#[derive(Debug, Clone)]
pub struct VariableRow {
    pub id: String,
    pub environment_id: Option<String>,
    pub key: String,
    pub value: String,
    pub enabled: bool,
}

impl VariableRow {
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            environment_id: row.get(1)?,
            key: row.get(2)?,
            value: row.get(3)?,
            enabled: row.get::<_, i64>(4)? != 0,
        })
    }
}

/// Raw row from the `history` table.
#[derive(Debug, Clone)]
pub struct HistoryRow {
    pub id: String,
    pub request_json: String,
    pub status_code: i64,
    pub status_text: String,
    pub response_time_ms: i64,
    pub response_size_bytes: i64,
    pub response_headers_json: String,
    pub response_body: String,
    pub executed_at: String,
}

impl HistoryRow {
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            request_json: row.get(1)?,
            status_code: row.get(2)?,
            status_text: row.get(3)?,
            response_time_ms: row.get(4)?,
            response_size_bytes: row.get(5)?,
            response_headers_json: row.get(6)?,
            response_body: row.get(7)?,
            executed_at: row.get(8)?,
        })
    }
}

/// Raw row from the `saved_responses` table.
#[derive(Debug, Clone)]
pub struct SavedResponseRow {
    pub id: String,
    pub request_id: String,
    pub label: String,
    pub status_code: i64,
    pub status_text: String,
    pub response_headers_json: String,
    pub response_body: String,
    pub response_time_ms: i64,
    pub response_size_bytes: i64,
    pub saved_at: String,
}

impl SavedResponseRow {
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            request_id: row.get(1)?,
            label: row.get(2)?,
            status_code: row.get(3)?,
            status_text: row.get(4)?,
            response_headers_json: row.get(5)?,
            response_body: row.get(6)?,
            response_time_ms: row.get(7)?,
            response_size_bytes: row.get(8)?,
            saved_at: row.get(9)?,
        })
    }
}

// ---------------------------------------------------------------------------
// Saved Responses CRUD
// ---------------------------------------------------------------------------

/// Insert a new saved_response row.
pub async fn insert_saved_response(
    handle: DbHandle,
    id: String,
    request_id: String,
    label: String,
    status_code: i64,
    status_text: String,
    response_headers_json: String,
    response_body: String,
    response_time_ms: i64,
    response_size_bytes: i64,
    saved_at: String,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let conn = handle.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO saved_responses
             (id, request_id, label, status_code, status_text,
              response_headers_json, response_body, response_time_ms, response_size_bytes, saved_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                id,
                request_id,
                label,
                status_code,
                status_text,
                response_headers_json,
                response_body,
                response_time_ms,
                response_size_bytes,
                saved_at,
            ],
        )
        .map_err(db_err)?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Return all saved responses for a specific request, ordered by `saved_at ASC`.
pub async fn list_saved_responses_by_request(
    handle: DbHandle,
    request_id: String,
) -> Result<Vec<SavedResponseRow>, String> {
    tokio::task::spawn_blocking(move || {
        let conn = handle.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id, request_id, label, status_code, status_text,
                        response_headers_json, response_body, response_time_ms, response_size_bytes, saved_at
                 FROM saved_responses WHERE request_id = ?1 ORDER BY saved_at ASC",
            )
            .map_err(db_err)?;

        let rows = stmt
            .query_map(params![request_id], SavedResponseRow::from_row)
            .map_err(db_err)?;

        rows.map(|r| r.map_err(db_err)).collect()
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Rename a saved response's label.
pub async fn rename_saved_response(
    handle: DbHandle,
    id: String,
    label: String,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let conn = handle.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE saved_responses SET label = ?1 WHERE id = ?2",
            params![label, id],
        )
        .map_err(db_err)?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Delete a single saved response by ID.
pub async fn delete_saved_response(handle: DbHandle, id: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let conn = handle.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "DELETE FROM saved_responses WHERE id = ?1",
            params![id],
        )
        .map_err(db_err)?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: open an in-memory database and run migrations.
    fn open_test_db() -> DbHandle {
        // ":memory:" uses an in-memory SQLite database — no file I/O.
        let conn = Connection::open_in_memory().expect("open in-memory DB");
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        run_migrations(&conn).expect("migrations should succeed");
        Arc::new(Mutex::new(conn))
    }

    #[test]
    fn migrations_are_idempotent() {
        // Running migrations twice on the same DB should not fail.
        let conn = Connection::open_in_memory().expect("open in-memory DB");
        run_migrations(&conn).expect("first migration run");
        run_migrations(&conn).expect("second migration run should be a no-op");
    }

    #[test]
    fn schema_migrations_table_records_version() {
        let conn = Connection::open_in_memory().expect("open in-memory DB");
        run_migrations(&conn).expect("migrations");
        let version: i64 = conn
            .query_row(
                "SELECT version FROM schema_migrations WHERE version = 1",
                [],
                |row| row.get(0),
            )
            .expect("version 1 should be recorded");
        assert_eq!(version, 1);
    }

    #[test]
    fn all_tables_exist_after_migration() {
        let handle = open_test_db();
        let conn = handle.lock().unwrap();

        let expected_tables = [
            "schema_migrations",
            "collections",
            "requests",
            "environments",
            "variables",
            "history",
        ];

        for table in &expected_tables {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    params![table],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            assert_eq!(count, 1, "table '{}' should exist", table);
        }
    }

    #[test]
    fn indexes_exist_after_migration() {
        let handle = open_test_db();
        let conn = handle.lock().unwrap();

        let expected_indexes = [
            "idx_requests_collection",
            "idx_variables_env",
            "idx_history_executed",
        ];

        for idx in &expected_indexes {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name=?1",
                    params![idx],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            assert_eq!(count, 1, "index '{}' should exist", idx);
        }
    }

    #[test]
    fn wal_mode_is_enabled_on_file_db() {
        // WAL pragma only makes sense on a file-backed database.
        let dir = std::env::temp_dir();
        let path = dir.join(format!("lightman_test_{}.db", uuid::Uuid::new_v4()));

        let handle = initialize_database(path.clone()).expect("initialize_database");
        let conn = handle.lock().unwrap();

        let mode: String = conn
            .query_row("PRAGMA journal_mode;", [], |row| row.get(0))
            .expect("journal_mode pragma");

        // Clean up the temp file.
        drop(conn);
        let _ = std::fs::remove_file(&path);

        assert_eq!(mode, "wal", "journal_mode should be WAL");
    }

    #[test]
    fn collections_enforces_name_length_constraint() {
        let handle = open_test_db();
        let conn = handle.lock().unwrap();

        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        // Empty name should be rejected by the CHECK constraint.
        let result = conn.execute(
            "INSERT INTO collections (id, name, position, created_at, updated_at) VALUES (?1, '', 0, ?2, ?3)",
            params![id, now, now],
        );
        assert!(result.is_err(), "empty name should violate CHECK constraint");

        // Name of 256 characters should also be rejected.
        let long_name = "a".repeat(256);
        let id2 = uuid::Uuid::new_v4().to_string();
        let result2 = conn.execute(
            "INSERT INTO collections (id, name, position, created_at, updated_at) VALUES (?1, ?2, 0, ?3, ?4)",
            params![id2, long_name, now, now],
        );
        assert!(result2.is_err(), "name >255 chars should violate CHECK constraint");
    }

    #[test]
    fn foreign_key_cascade_delete_works() {
        let handle = open_test_db();
        let conn = handle.lock().unwrap();

        let col_id = uuid::Uuid::new_v4().to_string();
        let req_id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        // Insert a collection.
        conn.execute(
            "INSERT INTO collections (id, name, position, created_at, updated_at) VALUES (?1, 'TestCol', 0, ?2, ?3)",
            params![col_id, now, now],
        ).unwrap();

        // Insert a request referencing that collection.
        conn.execute(
            "INSERT INTO requests (id, collection_id, name, method, url, created_at, updated_at) \
             VALUES (?1, ?2, 'TestReq', 'GET', 'http://example.com', ?3, ?4)",
            params![req_id, col_id, now, now],
        ).unwrap();

        // Delete the collection — the request should cascade-delete.
        conn.execute("DELETE FROM collections WHERE id = ?1", params![col_id]).unwrap();

        let remaining: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM requests WHERE id = ?1",
                params![req_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(remaining, 0, "request should be deleted by cascade");
    }

    // -------------------------------------------------------------------------
    // Task 2.2 — CRUD tests (async, using tokio::test)
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn insert_and_get_collection_roundtrip() {
        let handle = open_test_db();
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        insert_collection(
            handle.clone(),
            id.clone(),
            "My Collection".into(),
            Some("desc".into()),
            None,
            0,
            r#"{\\"type\\":\\"none\\"}"#.to_string(),
            now.clone(),
            now.clone(),
        )
        .await
        .unwrap();

        let row = get_collection(handle, id.clone())
            .await
            .unwrap()
            .expect("should find the inserted collection");

        assert_eq!(row.id, id);
        assert_eq!(row.name, "My Collection");
        assert_eq!(row.description, Some("desc".into()));
        assert!(row.parent_id.is_none());
    }

    #[tokio::test]
    async fn get_collection_returns_none_for_missing_id() {
        let handle = open_test_db();
        let result = get_collection(handle, "nonexistent-id".into())
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn list_collections_returns_all_inserted() {
        let handle = open_test_db();
        let now = chrono::Utc::now().to_rfc3339();

        for i in 0..3u64 {
            insert_collection(
                handle.clone(),
                uuid::Uuid::new_v4().to_string(),
                format!("Col {i}"),
                None,
                None,
                i as i64,
                r#"{\\"type\\":\\"none\\"}"#.to_string(),
                now.clone(),
                now.clone(),
            )
            .await
            .unwrap();
        }

        let rows = list_collections(handle).await.unwrap();
        assert_eq!(rows.len(), 3);
    }

    #[tokio::test]
    async fn update_collection_changes_name_and_description() {
        let handle = open_test_db();
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        insert_collection(
            handle.clone(),
            id.clone(),
            "Old Name".into(),
            None,
            None,
            0,
            r#"{\\"type\\":\\"none\\"}"#.to_string(),
            now.clone(),
            now.clone(),
        )
        .await
        .unwrap();

        let later = chrono::Utc::now().to_rfc3339();
        update_collection(
            handle.clone(),
            id.clone(),
            "New Name".into(),
            Some("new desc".into()),
            r#"{"type":"none"}"#.to_string(),later,
        )
        .await
        .unwrap();

        let row = get_collection(handle, id).await.unwrap().unwrap();
        assert_eq!(row.name, "New Name");
        assert_eq!(row.description, Some("new desc".into()));
    }

    #[tokio::test]
    async fn delete_collection_removes_row() {
        let handle = open_test_db();
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        insert_collection(
            handle.clone(),
            id.clone(),
            "Temp".into(),
            None,
            None,
            0,
            r#"{\\"type\\":\\"none\\"}"#.to_string(),
            now.clone(),
            now.clone(),
        )
        .await
        .unwrap();

        delete_collection(handle.clone(), id.clone()).await.unwrap();

        let result = get_collection(handle, id).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn delete_collection_cascades_to_requests() {
        let handle = open_test_db();
        let col_id = uuid::Uuid::new_v4().to_string();
        let req_id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        insert_collection(
            handle.clone(),
            col_id.clone(),
            "Parent".into(),
            None,
            None,
            0,
            r#"{\\"type\\":\\"none\\"}"#.to_string(),
            now.clone(),
            now.clone(),
        )
        .await
        .unwrap();

        insert_request(
            handle.clone(),
            req_id.clone(),
            col_id.clone(),
            "Req".into(),
            "GET".into(),
            "http://example.com".into(),
            "[]".into(),
            "[]".into(),
            r#"{"type":"none"}"#.into(),
            r#"{"type":"none"}"#.into(),
            0,
            now.clone(),
            now.clone(),
        )
        .await
        .unwrap();

        delete_collection(handle.clone(), col_id).await.unwrap();

        let result = get_request(handle, req_id).await.unwrap();
        assert!(result.is_none(), "request should be cascade-deleted");
    }

    #[tokio::test]
    async fn insert_and_get_request_roundtrip() {
        let handle = open_test_db();
        let col_id = uuid::Uuid::new_v4().to_string();
        let req_id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        insert_collection(
            handle.clone(),
            col_id.clone(),
            "Col".into(),
            None,
            None,
            0,
            r#"{\\"type\\":\\"none\\"}"#.to_string(),
            now.clone(),
            now.clone(),
        )
        .await
        .unwrap();

        insert_request(
            handle.clone(),
            req_id.clone(),
            col_id.clone(),
            "My Request".into(),
            "POST".into(),
            "https://api.example.com/data".into(),
            r#"[{"key":"Accept","value":"application/json","enabled":true}]"#.into(),
            "[]".into(),
            r#"{"type":"json","content":"{}"}"#.into(),
            r#"{"type":"none"}"#.into(),
            0,
            now.clone(),
            now.clone(),
        )
        .await
        .unwrap();

        let row = get_request(handle, req_id.clone())
            .await
            .unwrap()
            .expect("should find the inserted request");

        assert_eq!(row.id, req_id);
        assert_eq!(row.collection_id, col_id);
        assert_eq!(row.name, "My Request");
        assert_eq!(row.method, "POST");
        assert_eq!(row.url, "https://api.example.com/data");
    }

    #[tokio::test]
    async fn list_requests_by_collection_returns_in_position_order() {
        let handle = open_test_db();
        let col_id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        insert_collection(
            handle.clone(),
            col_id.clone(),
            "Col".into(),
            None,
            None,
            0,
            r#"{\\"type\\":\\"none\\"}"#.to_string(),
            now.clone(),
            now.clone(),
        )
        .await
        .unwrap();

        // Insert in reverse order so we can verify ORDER BY position.
        for pos in (0..3i64).rev() {
            insert_request(
                handle.clone(),
                uuid::Uuid::new_v4().to_string(),
                col_id.clone(),
                format!("Req {pos}"),
                "GET".into(),
                "http://x.com".into(),
                "[]".into(),
                "[]".into(),
                r#"{"type":"none"}"#.into(),
                r#"{"type":"none"}"#.into(),
                pos,
                now.clone(),
                now.clone(),
            )
            .await
            .unwrap();
        }

        let rows = list_requests_by_collection(handle, col_id).await.unwrap();
        assert_eq!(rows.len(), 3);
        assert!(
            rows.windows(2).all(|w| w[0].position <= w[1].position),
            "rows should be sorted by position ascending"
        );
    }

    #[tokio::test]
    async fn update_request_changes_fields() {
        let handle = open_test_db();
        let col_id = uuid::Uuid::new_v4().to_string();
        let req_id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        insert_collection(
            handle.clone(),
            col_id.clone(),
            "Col".into(),
            None,
            None,
            0,
            r#"{\\"type\\":\\"none\\"}"#.to_string(),
            now.clone(),
            now.clone(),
        )
        .await
        .unwrap();

        insert_request(
            handle.clone(),
            req_id.clone(),
            col_id,
            "Old".into(),
            "GET".into(),
            "http://old.com".into(),
            "[]".into(),
            "[]".into(),
            r#"{"type":"none"}"#.into(),
            r#"{"type":"none"}"#.into(),
            0,
            now.clone(),
            now.clone(),
        )
        .await
        .unwrap();

        let later = chrono::Utc::now().to_rfc3339();
        update_request(
            handle.clone(),
            req_id.clone(),
            "New".into(),
            "PUT".into(),
            "http://new.com".into(),
            "[]".into(),
            "[]".into(),
            r#"{"type":"none"}"#.into(),
            r#"{"type":"none"}"#.into(),
            1,
            later,
        )
        .await
        .unwrap();

        let row = get_request(handle, req_id).await.unwrap().unwrap();
        assert_eq!(row.name, "New");
        assert_eq!(row.method, "PUT");
        assert_eq!(row.url, "http://new.com");
        assert_eq!(row.position, 1);
    }

    #[tokio::test]
    async fn delete_request_removes_row() {
        let handle = open_test_db();
        let col_id = uuid::Uuid::new_v4().to_string();
        let req_id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        insert_collection(
            handle.clone(),
            col_id.clone(),
            "Col".into(),
            None,
            None,
            0,
            r#"{\\"type\\":\\"none\\"}"#.to_string(),
            now.clone(),
            now.clone(),
        )
        .await
        .unwrap();

        insert_request(
            handle.clone(),
            req_id.clone(),
            col_id,
            "Req".into(),
            "DELETE".into(),
            "http://x.com".into(),
            "[]".into(),
            "[]".into(),
            r#"{"type":"none"}"#.into(),
            r#"{"type":"none"}"#.into(),
            0,
            now.clone(),
            now.clone(),
        )
        .await
        .unwrap();

        delete_request(handle.clone(), req_id.clone()).await.unwrap();

        let result = get_request(handle, req_id).await.unwrap();
        assert!(result.is_none());
    }

    // -------------------------------------------------------------------------
    // Task 2.3 — Environment, Variable, and History CRUD tests
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn insert_and_list_environments() {
        let handle = open_test_db();
        let now = chrono::Utc::now().to_rfc3339();

        insert_environment(
            handle.clone(),
            uuid::Uuid::new_v4().to_string(),
            "Production".into(),
            false,
            now.clone(),
            now.clone(),
        )
        .await
        .unwrap();

        insert_environment(
            handle.clone(),
            uuid::Uuid::new_v4().to_string(),
            "Staging".into(),
            true,
            now.clone(),
            now.clone(),
        )
        .await
        .unwrap();

        let envs = list_environments(handle).await.unwrap();
        assert_eq!(envs.len(), 2);
        // Ordered by name: Production < Staging
        assert_eq!(envs[0].name, "Production");
        assert_eq!(envs[1].name, "Staging");
    }

    #[tokio::test]
    async fn get_active_environment_returns_active_one() {
        let handle = open_test_db();
        let now = chrono::Utc::now().to_rfc3339();
        let active_id = uuid::Uuid::new_v4().to_string();

        insert_environment(
            handle.clone(),
            uuid::Uuid::new_v4().to_string(),
            "Inactive".into(),
            false,
            now.clone(),
            now.clone(),
        )
        .await
        .unwrap();

        insert_environment(
            handle.clone(),
            active_id.clone(),
            "Active".into(),
            true,
            now.clone(),
            now.clone(),
        )
        .await
        .unwrap();

        let env = get_active_environment(handle)
            .await
            .unwrap()
            .expect("should find active environment");

        assert_eq!(env.id, active_id);
        assert!(env.is_active);
    }

    #[tokio::test]
    async fn get_active_environment_returns_none_when_no_active() {
        let handle = open_test_db();
        let now = chrono::Utc::now().to_rfc3339();

        insert_environment(
            handle.clone(),
            uuid::Uuid::new_v4().to_string(),
            "Dev".into(),
            false,
            now.clone(),
            now.clone(),
        )
        .await
        .unwrap();

        let result = get_active_environment(handle).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn update_environment_changes_name_and_active_flag() {
        let handle = open_test_db();
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        insert_environment(
            handle.clone(),
            id.clone(),
            "Old".into(),
            false,
            now.clone(),
            now.clone(),
        )
        .await
        .unwrap();

        let later = chrono::Utc::now().to_rfc3339();
        update_environment(handle.clone(), id.clone(), "New".into(), true, later)
            .await
            .unwrap();

        let env = get_active_environment(handle)
            .await
            .unwrap()
            .expect("should be active after update");

        assert_eq!(env.id, id);
        assert_eq!(env.name, "New");
        assert!(env.is_active);
    }

    #[tokio::test]
    async fn delete_environment_removes_row_and_cascades_variables() {
        let handle = open_test_db();
        let env_id = uuid::Uuid::new_v4().to_string();
        let var_id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        insert_environment(
            handle.clone(),
            env_id.clone(),
            "Temp".into(),
            false,
            now.clone(),
            now.clone(),
        )
        .await
        .unwrap();

        insert_variable(
            handle.clone(),
            var_id.clone(),
            Some(env_id.clone()),
            "BASE_URL".into(),
            "http://localhost".into(),
            true,
        )
        .await
        .unwrap();

        delete_environment(handle.clone(), env_id.clone()).await.unwrap();

        // Environment gone
        let envs = list_environments(handle.clone()).await.unwrap();
        assert!(!envs.iter().any(|e| e.id == env_id));

        // Variable also gone via cascade
        let vars = list_variables_by_env(handle, env_id).await.unwrap();
        assert!(vars.is_empty());
    }

    #[tokio::test]
    async fn insert_and_list_variables_by_env() {
        let handle = open_test_db();
        let env_id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        insert_environment(
            handle.clone(),
            env_id.clone(),
            "Dev".into(),
            false,
            now.clone(),
            now.clone(),
        )
        .await
        .unwrap();

        for key in &["API_KEY", "BASE_URL", "TIMEOUT"] {
            insert_variable(
                handle.clone(),
                uuid::Uuid::new_v4().to_string(),
                Some(env_id.clone()),
                key.to_string(),
                format!("value_{key}"),
                true,
            )
            .await
            .unwrap();
        }

        let vars = list_variables_by_env(handle, env_id).await.unwrap();
        assert_eq!(vars.len(), 3);
        // Ordered by key ASC: API_KEY, BASE_URL, TIMEOUT
        assert_eq!(vars[0].key, "API_KEY");
        assert_eq!(vars[1].key, "BASE_URL");
        assert_eq!(vars[2].key, "TIMEOUT");
    }

    #[tokio::test]
    async fn global_variables_have_null_environment_id() {
        let handle = open_test_db();

        // Insert a global variable (environment_id = None)
        insert_variable(
            handle.clone(),
            uuid::Uuid::new_v4().to_string(),
            None,
            "GLOBAL_VAR".into(),
            "global_value".into(),
            true,
        )
        .await
        .unwrap();

        // Verify it was stored with NULL environment_id
        let conn = {
            let h = handle.lock().unwrap();
            h.query_row(
                "SELECT COUNT(*) FROM variables WHERE environment_id IS NULL",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap()
        };
        assert_eq!(conn, 1);
    }

    #[tokio::test]
    async fn delete_variables_by_env_removes_only_that_env() {
        let handle = open_test_db();
        let env1 = uuid::Uuid::new_v4().to_string();
        let env2 = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        for (eid, name) in [(&env1, "Env1"), (&env2, "Env2")] {
            insert_environment(
                handle.clone(),
                eid.to_string(),
                name.into(),
                false,
                now.clone(),
                now.clone(),
            )
            .await
            .unwrap();
        }

        // Two vars in env1, one in env2
        for _ in 0..2 {
            insert_variable(
                handle.clone(),
                uuid::Uuid::new_v4().to_string(),
                Some(env1.clone()),
                "K".into(),
                "V".into(),
                true,
            )
            .await
            .unwrap();
        }
        insert_variable(
            handle.clone(),
            uuid::Uuid::new_v4().to_string(),
            Some(env2.clone()),
            "K2".into(),
            "V2".into(),
            true,
        )
        .await
        .unwrap();

        delete_variables_by_env(handle.clone(), env1.clone()).await.unwrap();

        let env1_vars = list_variables_by_env(handle.clone(), env1).await.unwrap();
        let env2_vars = list_variables_by_env(handle, env2).await.unwrap();

        assert!(env1_vars.is_empty(), "env1 variables should be deleted");
        assert_eq!(env2_vars.len(), 1, "env2 variable should be untouched");
    }

    #[tokio::test]
    async fn insert_and_list_history_ordered_desc() {
        let handle = open_test_db();

        // Insert entries with different timestamps
        for i in 0..3u64 {
            let executed_at = chrono::DateTime::<chrono::Utc>::from_timestamp(i as i64 * 1000, 0)
                .unwrap()
                .to_rfc3339();
            insert_history_entry(
                handle.clone(),
                uuid::Uuid::new_v4().to_string(),
                format!(r#"{{"url":"http://example.com/{i}"}}"#),
                200,
                "OK".into(),
                i as i64 * 10,
                512,
                "[]".into(),
                "ok".into(),
                executed_at,
            )
            .await
            .unwrap();
        }

        let entries = list_history(handle, None).await.unwrap();
        assert_eq!(entries.len(), 3);
        // Most recent first
        assert!(
            entries[0].executed_at >= entries[1].executed_at,
            "history should be ordered by executed_at DESC"
        );
        assert!(entries[1].executed_at >= entries[2].executed_at);
    }

    #[tokio::test]
    async fn list_history_with_limit() {
        let handle = open_test_db();
        let now = chrono::Utc::now().to_rfc3339();

        for _ in 0..5 {
            insert_history_entry(
                handle.clone(),
                uuid::Uuid::new_v4().to_string(),
                r#"{"url":"http://x.com"}"#.into(),
                200,
                "OK".into(),
                50,
                100,
                "[]".into(),
                "body".into(),
                now.clone(),
            )
            .await
            .unwrap();
        }

        let limited = list_history(handle, Some(3)).await.unwrap();
        assert_eq!(limited.len(), 3, "limit should cap returned rows");
    }

    #[tokio::test]
    async fn delete_history_entry_removes_single_row() {
        let handle = open_test_db();
        let id1 = uuid::Uuid::new_v4().to_string();
        let id2 = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        for id in [&id1, &id2] {
            insert_history_entry(
                handle.clone(),
                id.clone(),
                r#"{"url":"http://x.com"}"#.into(),
                200,
                "OK".into(),
                10,
                50,
                "[]".into(),
                "".into(),
                now.clone(),
            )
            .await
            .unwrap();
        }

        delete_history_entry(handle.clone(), id1.clone()).await.unwrap();

        let remaining = list_history(handle, None).await.unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, id2);
    }

    #[tokio::test]
    async fn clear_history_removes_all_rows() {
        let handle = open_test_db();
        let now = chrono::Utc::now().to_rfc3339();

        for _ in 0..4 {
            insert_history_entry(
                handle.clone(),
                uuid::Uuid::new_v4().to_string(),
                r#"{"url":"http://x.com"}"#.into(),
                200,
                "OK".into(),
                10,
                50,
                "[]".into(),
                "".into(),
                now.clone(),
            )
            .await
            .unwrap();
        }

        clear_history(handle.clone()).await.unwrap();

        let remaining = list_history(handle, None).await.unwrap();
        assert!(remaining.is_empty(), "all history rows should be deleted");
    }
}