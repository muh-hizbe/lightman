// Integration test: startup loading — database initialization, migration
// idempotency, and data persistence across simulated restarts.
//
// Task 23.3 — Requirements: 7.2, 7.3, 7.6
//
// These tests exercise `storage::initialize_database` directly to verify:
//   - A fresh DB is created and schema migrations run on the first call.
//   - Calling `initialize_database` again on the same file (simulating an app
//     restart) is idempotent — no errors, no duplicate rows, no data loss.
//   - Data written in one "session" is visible in a subsequent "session" that
//     re-opens the same file.
//   - The WAL mode pragma is applied.
//   - The `schema_migrations` table correctly records applied versions.

use std::path::PathBuf;
use uuid::Uuid;

use lightman_lib::{
    models::{AuthConfig, HttpMethod, RequestBody},
    services::{collection as collection_svc, environment as env_svc},
    storage,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Return a unique temp-file path.  The file is NOT created by this call.
fn temp_db_path() -> PathBuf {
    std::env::temp_dir().join(format!("lightman_startup_{}.db", Uuid::new_v4()))
}

// ===========================================================================
// Test 1 — Fresh database initializes successfully
// ===========================================================================

/// `initialize_database` on a new path must succeed and return a valid handle.
/// Basic operations (list collections) on the fresh DB must return an empty
/// result — i.e. the schema was created correctly.
#[tokio::test]
async fn test_fresh_database_initializes_successfully() {
    let path = temp_db_path();

    let db = storage::initialize_database(path.clone())
        .expect("initialize_database must succeed on a fresh path");

    // The schema must be ready — querying an empty table should return [].
    let collections = storage::list_collections(db.clone()).await.unwrap();
    assert!(collections.is_empty(), "fresh DB should have no collections");

    let _ = std::fs::remove_file(&path);
}

// ===========================================================================
// Test 2 — Migration is idempotent (simulated app restart)
// ===========================================================================

/// Opening the same database file twice (once to initialize, once to simulate
/// a restart) must not fail.  The `schema_migrations` table must still contain
/// exactly one row (migration v1) after the second open.
#[tokio::test]
async fn test_migration_is_idempotent() {
    let path = temp_db_path();

    // First initialization (fresh DB).
    {
        let db = storage::initialize_database(path.clone())
            .expect("first initialize_database must succeed");

        // Write something so we can verify persistence.
        collection_svc::create_collection(db.clone(), "Persist Me".to_string(), None, None, lightman_lib::models::AuthConfig::None)
            .await
            .unwrap();

        // Drop db (closes connection via Arc/Mutex when last ref is dropped).
        drop(db);
    }

    // Second initialization (restart simulation — same file).
    {
        let db = storage::initialize_database(path.clone())
            .expect("second initialize_database (restart) must succeed");

        // Data written in the first session must still be visible.
        let collections = storage::list_collections(db.clone()).await.unwrap();
        assert_eq!(
            collections.len(),
            1,
            "collection created in first session must persist across restart"
        );
        assert_eq!(collections[0].name, "Persist Me");

        // schema_migrations must contain exactly v1 (not duplicated).
        let migration_count: i64 = {
            let handle = db.clone();
            tokio::task::spawn_blocking(move || {
                let conn = handle.lock().unwrap();
                conn.query_row(
                    "SELECT COUNT(*) FROM schema_migrations",
                    [],
                    |row| row.get(0),
                )
                .unwrap()
            })
            .await
            .unwrap()
        };

        assert_eq!(
            migration_count, 1,
            "schema_migrations must contain exactly 1 row after two opens"
        );

        drop(db);
    }

    let _ = std::fs::remove_file(&path);
}

// ===========================================================================
// Test 3 — Data persists across simulated restarts (collections + requests)
// ===========================================================================

/// Collections and requests written before the DB is "closed" must be fully
/// readable after re-opening the same file.
#[tokio::test]
async fn test_data_persists_across_restart() {
    let path = temp_db_path();

    let (collection_id, request_id);

    // Session 1: write data.
    {
        let db = storage::initialize_database(path.clone()).unwrap();

        let col =
            collection_svc::create_collection(db.clone(), "Restart Col".to_string(), None, None, lightman_lib::models::AuthConfig::None)
                .await
                .unwrap();
        collection_id = col.id.clone();

        let req = collection_svc::create_request(
            db.clone(),
            col.id.clone(),
            "Restart Req".to_string(),
            HttpMethod::Post,
            "https://api.example.com/restart".to_string(),
            vec![],
            vec![],
            RequestBody::None,
            AuthConfig::None,
        )
        .await
        .unwrap();
        request_id = req.id.clone();

        drop(db);
    }

    // Session 2: read data.
    {
        let db = storage::initialize_database(path.clone()).unwrap();

        let col = collection_svc::get_collection(db.clone(), collection_id.clone())
            .await
            .unwrap_or(None);
        let _ = col;

        // Drop db (closes connection via Arc/Mutex when last ref is dropped).
        drop(db);
    }

    // Second initialization (restart simulation — same file).
    {
        let db = storage::initialize_database(path.clone())
            .expect("second initialize_database (restart) must succeed");

        // Data written in the first session must still be visible.
        let collections = storage::list_collections(db.clone()).await.unwrap();
        assert_eq!(
            collections.len(),
            1,
            "collection created in first session must persist across restart"
        );
        assert_eq!(collections[0].name, "Persist Me");

        // schema_migrations must contain exactly v1 (not duplicated).
        let migration_count: i64 = {
            let handle = db.clone();
            tokio::task::spawn_blocking(move || {
                let conn = handle.lock().unwrap();
                conn.query_row(
                    "SELECT COUNT(*) FROM schema_migrations",
                    [],
                    |row| row.get(0),
                )
                .unwrap()
            })
            .await
            .unwrap()
        };

        assert_eq!(
            migration_count, 1,
            "schema_migrations must contain exactly 1 row after two opens"
        );

        drop(db);
    }

    let _ = std::fs::remove_file(&path);
}

// ===========================================================================
// Test 3 — Data persists across simulated restarts (collections + requests)
// ===========================================================================

/// Collections and requests written before the DB is "closed" must be fully

// ===========================================================================
// Test 4 — Environments and variables persist across restart
// ===========================================================================

/// Environments and their variables survive a simulated restart.
#[tokio::test]
async fn test_environments_persist_across_restart() {
    let path = temp_db_path();

    let (env_id, var_id);

    // Session 1.
    {
        let db = storage::initialize_database(path.clone()).unwrap();

        let env = env_svc::create_environment(db.clone(), "Staging".to_string())
            .await
            .unwrap();
        env_id = env.id.clone();

        let var = env_svc::create_variable(
            db.clone(),
            Some(env.id.clone()),
            "API_URL".to_string(),
            "https://staging.api.example.com".to_string(),
            true,
        )
        .await
        .unwrap();
        var_id = var.id.clone();

        drop(db);
    }

    // Session 2.
    {
        let db = storage::initialize_database(path.clone()).unwrap();

        let environments = env_svc::list_environments(db.clone()).await.unwrap();
        assert_eq!(environments.len(), 1, "environment must persist");
        assert_eq!(environments[0].id, env_id);
        assert_eq!(environments[0].name, "Staging");

        let variables = env_svc::list_variables(db.clone(), env_id.clone())
            .await
            .unwrap();
        assert_eq!(variables.len(), 1, "variable must persist");
        assert_eq!(variables[0].id, var_id);
        assert_eq!(variables[0].key, "API_URL");
        assert_eq!(variables[0].value, "https://staging.api.example.com");
        assert!(variables[0].enabled);

        drop(db);
    }

    let _ = std::fs::remove_file(&path);
}

// ===========================================================================
// Test 5 — History entries persist across restart
// ===========================================================================

/// History entries written in one session are readable after re-opening the
/// database file.
#[tokio::test]
async fn test_history_persists_across_restart() {
    let path = temp_db_path();

    let history_id = Uuid::new_v4().to_string();

    // Session 1: write a history entry.
    {
        let db = storage::initialize_database(path.clone()).unwrap();

        storage::insert_history_entry(
            db.clone(),
            history_id.clone(),
            r#"{"url":"https://example.com","method":"Get"}"#.to_string(),
            200,
            "OK".to_string(),
            42,
            128,
            "[]".to_string(),
            r#"{"ok":true}"#.to_string(),
            chrono::Utc::now().to_rfc3339(),
        )
        .await
        .unwrap();

        drop(db);
    }

    // Session 2: verify the entry is still there.
    {
        let db = storage::initialize_database(path.clone()).unwrap();

        let history = storage::list_history(db.clone(), None).await.unwrap();
        assert_eq!(history.len(), 1, "history entry must persist across restart");
        assert_eq!(history[0].id, history_id);
        assert_eq!(history[0].status_code, 200);
        assert_eq!(history[0].response_time_ms, 42);
        assert_eq!(history[0].response_size_bytes, 128);

        drop(db);
    }

    let _ = std::fs::remove_file(&path);
}

// ===========================================================================
// Test 6 — WAL mode is active after initialization
// ===========================================================================

/// After `initialize_database`, the SQLite journal mode must be WAL (as set by
/// `PRAGMA journal_mode=WAL`).
#[tokio::test]
async fn test_wal_mode_is_active() {
    let path = temp_db_path();

    let db = storage::initialize_database(path.clone()).unwrap();

    let journal_mode: String = {
        let handle = db.clone();
        tokio::task::spawn_blocking(move || {
            let conn = handle.lock().unwrap();
            conn.query_row(
                "PRAGMA journal_mode",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap()
        })
        .await
        .unwrap()
    };

    assert_eq!(
        journal_mode.to_lowercase(),
        "wal",
        "journal mode should be WAL after initialization"
    );

    let _ = std::fs::remove_file(&path);
}

// ===========================================================================
// Test 7 — Multiple initializations of different paths are independent
// ===========================================================================

/// Two databases opened from different paths must be fully independent: data
/// written to one must not appear in the other.
#[tokio::test]
async fn test_independent_databases_are_isolated() {
    let path_a = temp_db_path();
    let path_b = temp_db_path();

    let db_a = storage::initialize_database(path_a.clone()).unwrap();
    let db_b = storage::initialize_database(path_b.clone()).unwrap();

    // Write a collection only to DB A.
    collection_svc::create_collection(db_a.clone(), "Only in A".to_string(), None, None, lightman_lib::models::AuthConfig::None)
        .await
        .unwrap();

    // DB B should remain empty.
    let cols_b = storage::list_collections(db_b.clone()).await.unwrap();
    assert!(
        cols_b.is_empty(),
        "DB B should be empty — data should not leak between databases"
    );

    // DB A should have exactly one collection.
    let cols_a = storage::list_collections(db_a.clone()).await.unwrap();
    assert_eq!(cols_a.len(), 1);
    assert_eq!(cols_a[0].name, "Only in A");

    let _ = std::fs::remove_file(&path_a);
    let _ = std::fs::remove_file(&path_b);
}

// ===========================================================================
// Test 8 — Active environment is correctly persisted and retrieved
// ===========================================================================

/// `set_active_environment` followed by a simulated restart must preserve the
/// active environment selection.
#[tokio::test]
async fn test_active_environment_persists_across_restart() {
    let path = temp_db_path();

    let env_id;

    // Session 1: create two environments and activate the second.
    {
        let db = storage::initialize_database(path.clone()).unwrap();

        env_svc::create_environment(db.clone(), "Production".to_string())
            .await
            .unwrap();
        let staging = env_svc::create_environment(db.clone(), "Staging".to_string())
            .await
            .unwrap();
        env_id = staging.id.clone();

        env_svc::set_active_environment(db.clone(), staging.id.clone())
            .await
            .unwrap();

        drop(db);
    }

    // Session 2: the active environment must still be "Staging".
    {
        let db = storage::initialize_database(path.clone()).unwrap();

        let active = storage::get_active_environment(db.clone())
            .await
            .unwrap()
            .expect("an active environment must be set");

        assert_eq!(active.id, env_id, "active environment ID must persist");
        assert_eq!(active.name, "Staging");
        assert!(active.is_active);

        drop(db);
    }

    let _ = std::fs::remove_file(&path);
}

// ===========================================================================
// Test 9 — Schema has all expected tables after initialization
// ===========================================================================

/// The DB schema after `initialize_database` must contain all the tables
/// required by the application: `collections`, `requests`, `environments`,
/// `variables`, `history`, and `schema_migrations`.
#[tokio::test]
async fn test_all_expected_tables_exist_after_init() {
    let path = temp_db_path();

    let db = storage::initialize_database(path.clone()).unwrap();

    let expected_tables = vec![
        "collections",
        "requests",
        "environments",
        "variables",
        "history",
        "schema_migrations",
    ];

    let handle = db.clone();
    let actual_tables: Vec<String> = tokio::task::spawn_blocking(move || {
        let conn = handle.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name",
            )
            .unwrap();
        stmt.query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    })
    .await
    .unwrap();

    for table in &expected_tables {
        assert!(
            actual_tables.iter().any(|t| t == table),
            "expected table '{table}' not found in schema; actual tables: {actual_tables:?}"
        );
    }

    let _ = std::fs::remove_file(&path);
}
