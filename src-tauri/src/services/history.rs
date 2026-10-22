// History service — manage and query request execution history.
// Task 10.1: save_entry, list_entries, search_entries, delete_entry, clear_all.
//
// Requirements: 4.1, 4.2, 4.3, 4.5

use crate::models::{Header, HistoryEntry, HttpMethod, Request};
use crate::storage::{self, DbHandle, HistoryRow};

// ---------------------------------------------------------------------------
// Row → Domain model converter
// ---------------------------------------------------------------------------

/// Convert a `HistoryRow` from storage into a `HistoryEntry` domain model by
/// deserialising the JSON columns.
fn history_entry_from_row(row: HistoryRow) -> Result<HistoryEntry, String> {
    let request: Request = serde_json::from_str(&row.request_json)
        .map_err(|e| format!("Failed to parse request_json: {e}"))?;

    let response_headers: Vec<Header> = serde_json::from_str(&row.response_headers_json)
        .map_err(|e| format!("Failed to parse response_headers_json: {e}"))?;

    Ok(HistoryEntry {
        id: row.id,
        request,
        status_code: row.status_code as u16,
        status_text: row.status_text,
        response_time_ms: row.response_time_ms as u64,
        response_size_bytes: row.response_size_bytes as u64,
        response_headers,
        response_body: row.response_body,
        executed_at: row.executed_at,
    })
}

// ---------------------------------------------------------------------------
// Public service functions
// ---------------------------------------------------------------------------

/// Persist a `HistoryEntry` to storage.
///
/// Serialises the embedded `Request` and response headers to JSON before
/// writing to the `history` table.
///
/// Requirement 4.1.
pub async fn save_entry(db: DbHandle, entry: HistoryEntry) -> Result<(), String> {
    let request_json = serde_json::to_string(&entry.request)
        .map_err(|e| format!("Failed to serialise request: {e}"))?;

    let response_headers_json = serde_json::to_string(&entry.response_headers)
        .map_err(|e| format!("Failed to serialise response_headers: {e}"))?;

    storage::insert_history_entry(
        db,
        entry.id,
        request_json,
        entry.status_code as i64,
        entry.status_text,
        entry.response_time_ms as i64,
        entry.response_size_bytes as i64,
        response_headers_json,
        entry.response_body,
        entry.executed_at,
    )
    .await
}

/// Return all history entries ordered by `executed_at DESC` (most recent first).
///
/// Pass `limit` to cap the number of results; `None` returns all entries.
///
/// Requirement 4.2.
pub async fn list_entries(
    db: DbHandle,
    limit: Option<i64>,
) -> Result<Vec<HistoryEntry>, String> {
    let rows = storage::list_history(db, limit).await?;
    rows.into_iter().map(history_entry_from_row).collect()
}

/// Search history entries by URL substring or HTTP method.
///
/// - `url_filter`: if `Some`, only entries whose request URL contains this
///   substring (case-insensitive) are included.
/// - `method_filter`: if `Some`, only entries whose request method matches
///   this `HttpMethod` exactly are included.
///
/// Both filters are ANDed together when both are `Some`.
/// Results are returned in `executed_at DESC` order.
///
/// Requirement 4.3.
pub async fn search_entries(
    db: DbHandle,
    url_filter: Option<String>,
    method_filter: Option<HttpMethod>,
) -> Result<Vec<HistoryEntry>, String> {
    // Fetch all entries and filter in-memory.
    // The history table stores request data as JSON, so SQL-level filtering
    // on JSON fields would require SQLite JSON functions that may not always
    // be available. In-memory filtering keeps things portable and simple;
    // the history table is expected to stay manageable in size.
    let rows = storage::list_history(db, None).await?;

    let mut entries: Vec<HistoryEntry> = rows
        .into_iter()
        .map(history_entry_from_row)
        .collect::<Result<Vec<_>, _>>()?;

    // Apply URL substring filter (case-insensitive).
    if let Some(ref url_query) = url_filter {
        let lower_query = url_query.to_lowercase();
        entries.retain(|e| e.request.url.to_lowercase().contains(&lower_query));
    }

    // Apply HTTP method filter (exact match).
    if let Some(ref method) = method_filter {
        entries.retain(|e| &e.request.method == method);
    }

    Ok(entries)
}

/// Delete a single history entry by its UUID.
///
/// Returns `Err` if the entry does not exist.
///
/// Requirement 4.5.
pub async fn delete_entry(db: DbHandle, id: String) -> Result<(), String> {
    storage::delete_history_entry(db, id).await
}

/// Delete all history entries.
///
/// Requirement 4.5.
pub async fn clear_all(db: DbHandle) -> Result<(), String> {
    storage::clear_history(db).await
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AuthConfig, HttpMethod, RequestBody};
    use std::sync::{Arc, Mutex};

    fn make_db() -> DbHandle {
        use rusqlite::Connection;
        let conn = Connection::open_in_memory().expect("in-memory DB");
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        crate::storage::run_migrations_for_test(&conn).expect("migrations");
        Arc::new(Mutex::new(conn))
    }

    fn make_request(id: &str, method: HttpMethod, url: &str) -> Request {
        Request {
            id: id.to_string(),
            collection_id: "col-1".to_string(),
            name: "test request".to_string(),
            method,
            url: url.to_string(),
            headers: vec![],
            params: vec![],
            body: RequestBody::None,
            auth: AuthConfig::None,
            position: 0,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        }
    }

    fn make_entry(id: &str, method: HttpMethod, url: &str, executed_at: &str) -> HistoryEntry {
        HistoryEntry {
            id: id.to_string(),
            request: make_request(id, method, url),
            status_code: 200,
            status_text: "OK".to_string(),
            response_time_ms: 50,
            response_size_bytes: 128,
            response_headers: vec![],
            response_body: "{}".to_string(),
            executed_at: executed_at.to_string(),
        }
    }

    // ---- save + list -------------------------------------------------------

    #[tokio::test]
    async fn save_and_list_entries_round_trip() {
        let db = make_db();
        let entry = make_entry(
            "hist-1",
            HttpMethod::Get,
            "https://example.com",
            "2024-01-01T00:00:00Z",
        );

        save_entry(db.clone(), entry.clone()).await.unwrap();

        let result = list_entries(db, None).await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "hist-1");
        assert_eq!(result[0].request.url, "https://example.com");
        assert_eq!(result[0].status_code, 200);
    }

    #[tokio::test]
    async fn list_entries_ordered_by_executed_at_desc() {
        let db = make_db();

        save_entry(
            db.clone(),
            make_entry("h1", HttpMethod::Get, "https://a.com", "2024-01-01T00:00:00Z"),
        )
        .await
        .unwrap();

        save_entry(
            db.clone(),
            make_entry("h2", HttpMethod::Post, "https://b.com", "2024-01-03T00:00:00Z"),
        )
        .await
        .unwrap();

        save_entry(
            db.clone(),
            make_entry("h3", HttpMethod::Put, "https://c.com", "2024-01-02T00:00:00Z"),
        )
        .await
        .unwrap();

        let result = list_entries(db, None).await.unwrap();
        // Most recent first.
        assert_eq!(result[0].id, "h2");
        assert_eq!(result[1].id, "h3");
        assert_eq!(result[2].id, "h1");
    }

    #[tokio::test]
    async fn list_entries_with_limit() {
        let db = make_db();

        for i in 0..5u8 {
            save_entry(
                db.clone(),
                make_entry(
                    &format!("h{i}"),
                    HttpMethod::Get,
                    "https://example.com",
                    &format!("2024-01-0{}T00:00:00Z", i + 1),
                ),
            )
            .await
            .unwrap();
        }

        let result = list_entries(db, Some(3)).await.unwrap();
        assert_eq!(result.len(), 3);
    }

    // ---- search ------------------------------------------------------------

    #[tokio::test]
    async fn search_by_url_substring_case_insensitive() {
        let db = make_db();

        save_entry(
            db.clone(),
            make_entry("h1", HttpMethod::Get, "https://api.example.com/users", "2024-01-01T00:00:00Z"),
        )
        .await
        .unwrap();

        save_entry(
            db.clone(),
            make_entry("h2", HttpMethod::Post, "https://api.example.com/posts", "2024-01-02T00:00:00Z"),
        )
        .await
        .unwrap();

        save_entry(
            db.clone(),
            make_entry("h3", HttpMethod::Get, "https://other.com/items", "2024-01-03T00:00:00Z"),
        )
        .await
        .unwrap();

        let result = search_entries(db, Some("EXAMPLE".to_string()), None)
            .await
            .unwrap();

        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|e| e.request.url.contains("example.com")));
    }

    #[tokio::test]
    async fn search_by_method_filter() {
        let db = make_db();

        save_entry(
            db.clone(),
            make_entry("h1", HttpMethod::Get, "https://a.com", "2024-01-01T00:00:00Z"),
        )
        .await
        .unwrap();

        save_entry(
            db.clone(),
            make_entry("h2", HttpMethod::Post, "https://b.com", "2024-01-02T00:00:00Z"),
        )
        .await
        .unwrap();

        save_entry(
            db.clone(),
            make_entry("h3", HttpMethod::Get, "https://c.com", "2024-01-03T00:00:00Z"),
        )
        .await
        .unwrap();

        let result = search_entries(db, None, Some(HttpMethod::Get))
            .await
            .unwrap();

        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|e| e.request.method == HttpMethod::Get));
    }

    #[tokio::test]
    async fn search_combined_url_and_method() {
        let db = make_db();

        save_entry(
            db.clone(),
            make_entry("h1", HttpMethod::Get, "https://api.example.com/users", "2024-01-01T00:00:00Z"),
        )
        .await
        .unwrap();

        save_entry(
            db.clone(),
            make_entry("h2", HttpMethod::Post, "https://api.example.com/users", "2024-01-02T00:00:00Z"),
        )
        .await
        .unwrap();

        save_entry(
            db.clone(),
            make_entry("h3", HttpMethod::Get, "https://other.com/users", "2024-01-03T00:00:00Z"),
        )
        .await
        .unwrap();

        // Only GET requests to example.com.
        let result = search_entries(
            db,
            Some("example.com".to_string()),
            Some(HttpMethod::Get),
        )
        .await
        .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "h1");
    }

    #[tokio::test]
    async fn search_no_filters_returns_all() {
        let db = make_db();

        for i in 0..3u8 {
            save_entry(
                db.clone(),
                make_entry(
                    &format!("h{i}"),
                    HttpMethod::Get,
                    "https://example.com",
                    &format!("2024-01-0{}T00:00:00Z", i + 1),
                ),
            )
            .await
            .unwrap();
        }

        let result = search_entries(db, None, None).await.unwrap();
        assert_eq!(result.len(), 3);
    }

    // ---- delete_entry ------------------------------------------------------

    #[tokio::test]
    async fn delete_entry_removes_single_record() {
        let db = make_db();

        save_entry(
            db.clone(),
            make_entry("h1", HttpMethod::Get, "https://a.com", "2024-01-01T00:00:00Z"),
        )
        .await
        .unwrap();

        save_entry(
            db.clone(),
            make_entry("h2", HttpMethod::Post, "https://b.com", "2024-01-02T00:00:00Z"),
        )
        .await
        .unwrap();

        delete_entry(db.clone(), "h1".to_string()).await.unwrap();

        let result = list_entries(db, None).await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "h2");
    }

    // ---- clear_all ---------------------------------------------------------

    #[tokio::test]
    async fn clear_all_removes_all_entries() {
        let db = make_db();

        for i in 0..3u8 {
            save_entry(
                db.clone(),
                make_entry(
                    &format!("h{i}"),
                    HttpMethod::Get,
                    "https://example.com",
                    &format!("2024-01-0{}T00:00:00Z", i + 1),
                ),
            )
            .await
            .unwrap();
        }

        clear_all(db.clone()).await.unwrap();

        let result = list_entries(db, None).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn clear_all_on_empty_table_is_a_noop() {
        let db = make_db();
        // Should not error on an already-empty table.
        clear_all(db.clone()).await.unwrap();
        let result = list_entries(db, None).await.unwrap();
        assert!(result.is_empty());
    }
}
