// Update commands — two-step flow: check first, then install on demand.
//
// check_update  → fetches latest.json, returns whether an update is available
//                 along with the new version string. Does not download anything.
// install_update → downloads and installs the pending update, then restarts.
//                  Must be called after check_update has confirmed availability.

use std::sync::Mutex;
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

/// Shared state that holds the pending update between the check and install
/// steps. Wrapped in a Mutex so it can be stored in Tauri's managed state.
pub struct PendingUpdate(pub Mutex<Option<Update>>);

#[derive(serde::Serialize)]
pub struct UpdateInfo {
    pub available: bool,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub release_notes: Option<String>,
}

/// Returns the current app version read from tauri.conf.json at build time.
#[tauri::command]
pub fn get_app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

/// Checks GitHub releases for a newer version.
/// Does NOT download anything — just returns availability info and stores the
/// pending `Update` object in managed state for `install_update` to consume.
#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<UpdateInfo, String> {
    let current = app.package_info().version.to_string();

    let updater = app
        .updater_builder()
        .build()
        .map_err(|e| format!("Failed to build updater: {e}"))?;

    let maybe_update = updater
        .check()
        .await
        .map_err(|e| format!("Update check failed: {e}"))?;

    match maybe_update {
        Some(update) => {
            let latest = update.version.clone();
            let notes = update.body.clone();

            // Stash the update object so install_update can use it later.
            *app.state::<PendingUpdate>().0.lock().unwrap() = Some(update);

            Ok(UpdateInfo {
                available: true,
                current_version: current,
                latest_version: Some(latest),
                release_notes: notes,
            })
        }
        None => {
            // Clear any stale pending update.
            *app.state::<PendingUpdate>().0.lock().unwrap() = None;

            Ok(UpdateInfo {
                available: false,
                current_version: current,
                latest_version: None,
                release_notes: None,
            })
        }
    }
}

/// Downloads and installs the update that was found by `check_update`,
/// then restarts the app. Errors if called before a successful `check_update`.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    let update = app
        .state::<PendingUpdate>()
        .0
        .lock()
        .unwrap()
        .take()
        .ok_or_else(|| "No pending update — run check_update first".to_string())?;

    update
        .download_and_install(|_chunk, _total| {}, || {})
        .await
        .map_err(|e| format!("Update installation failed: {e}"))?;

    app.restart();
}
