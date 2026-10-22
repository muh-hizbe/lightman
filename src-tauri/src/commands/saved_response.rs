// Tauri IPC commands for saved responses.
// Allows the frontend to save, list, rename, and delete response snapshots
// associated with a specific HTTP request.

use tauri::State;

use crate::models::{Header, SavedResponse};
use crate::services;
use crate::storage::DbHandle;

// ---------------------------------------------------------------------------
// save_response
// ---------------------------------------------------------------------------

/// Save a response snapshot for the given request.
///
/// # Frontend call
/// ```ts
/// invoke<SavedResponse>('save_response', {
///   requestId, label, statusCode, statusText,
///   headers, body, responseTimeMs, responseSizeBytes
/// })
/// ```
#[tauri::command]
pub async fn save_response(
    db: State<'_, DbHandle>,
    request_id: String,
    label: String,
    status_code: u16,
    status_text: String,
    headers: Vec<Header>,
    body: String,
    response_time_ms: u64,
    response_size_bytes: u64,
) -> Result<SavedResponse, String> {
    services::saved_response::save_response(
        db.inner().clone(),
        request_id,
        label,
        status_code,
        status_text,
        headers,
        body,
        response_time_ms,
        response_size_bytes,
    )
    .await
}

// ---------------------------------------------------------------------------
// list_saved_responses
// ---------------------------------------------------------------------------

/// List all saved responses for a specific request.
///
/// # Frontend call
/// ```ts
/// invoke<SavedResponse[]>('list_saved_responses', { requestId })
/// ```
#[tauri::command]
pub async fn list_saved_responses(
    db: State<'_, DbHandle>,
    request_id: String,
) -> Result<Vec<SavedResponse>, String> {
    services::saved_response::list_saved_responses(db.inner().clone(), request_id).await
}

// ---------------------------------------------------------------------------
// rename_saved_response
// ---------------------------------------------------------------------------

/// Rename an existing saved response.
///
/// # Frontend call
/// ```ts
/// invoke<void>('rename_saved_response', { id, newLabel })
/// ```
#[tauri::command]
pub async fn rename_saved_response(
    db: State<'_, DbHandle>,
    id: String,
    new_label: String,
) -> Result<(), String> {
    services::saved_response::rename_saved_response(db.inner().clone(), id, new_label).await
}

// ---------------------------------------------------------------------------
// delete_saved_response
// ---------------------------------------------------------------------------

/// Delete a saved response by ID.
///
/// # Frontend call
/// ```ts
/// invoke<void>('delete_saved_response', { id })
/// ```
#[tauri::command]
pub async fn delete_saved_response(
    db: State<'_, DbHandle>,
    id: String,
) -> Result<(), String> {
    services::saved_response::delete_saved_response(db.inner().clone(), id).await
}
