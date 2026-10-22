// Business logic for saved responses.
// Handles save, list, rename, and delete operations.

use chrono::Utc;
use uuid::Uuid;

use crate::models::{Header, SavedResponse};
use crate::storage::{self, DbHandle};

/// Save a new response snapshot for the given request.
///
/// `label` defaults to `"Response"` if an empty string is provided; the
/// caller should supply a meaningful name (e.g. `"Response 1"`) generated
/// from the existing count.
pub async fn save_response(
    db: DbHandle,
    request_id: String,
    label: String,
    status_code: u16,
    status_text: String,
    headers: Vec<Header>,
    body: String,
    response_time_ms: u64,
    response_size_bytes: u64,
) -> Result<SavedResponse, String> {
    let id = Uuid::new_v4().to_string();
    let saved_at = Utc::now().to_rfc3339();

    let headers_json = serde_json::to_string(&headers)
        .map_err(|e| format!("Failed to serialise headers: {e}"))?;

    let effective_label = if label.trim().is_empty() {
        "Response".to_string()
    } else {
        label.trim().to_string()
    };

    storage::insert_saved_response(
        db,
        id.clone(),
        request_id.clone(),
        effective_label.clone(),
        status_code as i64,
        status_text.clone(),
        headers_json,
        body.clone(),
        response_time_ms as i64,
        response_size_bytes as i64,
        saved_at.clone(),
    )
    .await?;

    Ok(SavedResponse {
        id,
        request_id,
        label: effective_label,
        status_code,
        status_text,
        headers,
        body,
        response_time_ms,
        response_size_bytes,
        saved_at,
    })
}

/// List all saved responses for a request, ordered by `saved_at ASC`.
pub async fn list_saved_responses(
    db: DbHandle,
    request_id: String,
) -> Result<Vec<SavedResponse>, String> {
    let rows = storage::list_saved_responses_by_request(db, request_id).await?;

    rows.into_iter()
        .map(|row| {
            let headers: Vec<Header> = serde_json::from_str(&row.response_headers_json)
                .unwrap_or_default();

            Ok(SavedResponse {
                id: row.id,
                request_id: row.request_id,
                label: row.label,
                status_code: row.status_code as u16,
                status_text: row.status_text,
                headers,
                body: row.response_body,
                response_time_ms: row.response_time_ms as u64,
                response_size_bytes: row.response_size_bytes as u64,
                saved_at: row.saved_at,
            })
        })
        .collect()
}

/// Rename an existing saved response.
pub async fn rename_saved_response(
    db: DbHandle,
    id: String,
    new_label: String,
) -> Result<(), String> {
    if new_label.trim().is_empty() {
        return Err("Label cannot be empty".to_string());
    }
    storage::rename_saved_response(db, id, new_label.trim().to_string()).await
}

/// Delete a saved response by ID.
pub async fn delete_saved_response(db: DbHandle, id: String) -> Result<(), String> {
    storage::delete_saved_response(db, id).await
}
