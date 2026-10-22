// Tauri IPC commands for built-in sample requests.
//
// Task 12.2: Expose `list_samples` as a #[tauri::command] async function.
//
// Requirements: 10.2, 10.4, 8.3

use crate::models::Request;
use crate::services::sample as svc;

// ---------------------------------------------------------------------------
// Sample commands
// ---------------------------------------------------------------------------

/// Return the static list of built-in sample requests.
///
/// These are read-only presets provided to help users get started quickly.
/// No database access is required — the list is generated from static data
/// in the sample service.
///
/// # Frontend call
/// ```ts
/// invoke<Request[]>('list_samples')
/// ```
#[tauri::command]
pub async fn list_samples() -> Result<Vec<Request>, String> {
    Ok(svc::list_samples())
}
