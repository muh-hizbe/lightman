// Tauri IPC commands for request history.
// These are the entry points called by the React frontend via invoke().
//
// Task 10.2: Expose history operations as #[tauri::command] async functions.
//
// Requirements: 4.2, 4.3, 4.5, 8.3

use crate::models::{HistoryEntry, HttpMethod};
use crate::services::history as svc;
use crate::storage::DbHandle;
use tauri::State;

// ---------------------------------------------------------------------------
// History commands
// ---------------------------------------------------------------------------

/// Return history entries ordered by `executed_at DESC` (most recent first).
///
/// Pass `limit` to cap the number of results; omit (or pass `null`) to return
/// all entries.
///
/// # Frontend call
/// ```ts
/// invoke<HistoryEntry[]>('list_history_entries', { limit })
/// ```
#[tauri::command]
pub async fn list_history_entries(
    db: State<'_, DbHandle>,
    limit: Option<i64>,
) -> Result<Vec<HistoryEntry>, String> {
    svc::list_entries(db.inner().clone(), limit).await
}

/// Search history entries by URL substring and/or HTTP method.
///
/// - `url_filter`: if provided, only entries whose request URL contains this
///   substring (case-insensitive) are included.
/// - `method_filter`: if provided, only entries whose HTTP method matches
///   exactly are included.
///
/// Both filters are ANDed together when both are supplied.
/// Results are ordered by `executed_at DESC`.
///
/// # Frontend call
/// ```ts
/// invoke<HistoryEntry[]>('search_history_entries', { urlFilter, methodFilter })
/// ```
#[tauri::command]
pub async fn search_history_entries(
    db: State<'_, DbHandle>,
    url_filter: Option<String>,
    method_filter: Option<HttpMethod>,
) -> Result<Vec<HistoryEntry>, String> {
    svc::search_entries(db.inner().clone(), url_filter, method_filter).await
}

/// Delete a single history entry by its UUID.
///
/// Returns an error if the entry does not exist.
///
/// # Frontend call
/// ```ts
/// invoke<void>('delete_history_entry', { id })
/// ```
#[tauri::command]
pub async fn delete_history_entry(
    db: State<'_, DbHandle>,
    id: String,
) -> Result<(), String> {
    svc::delete_entry(db.inner().clone(), id).await
}

/// Delete all history entries.
///
/// # Frontend call
/// ```ts
/// invoke<void>('clear_history')
/// ```
#[tauri::command]
pub async fn clear_history(db: State<'_, DbHandle>) -> Result<(), String> {
    svc::clear_all(db.inner().clone()).await
}
