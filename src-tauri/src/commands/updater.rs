// Update commands — check for a new release and install it.
//
// Uses tauri-plugin-updater to fetch the latest release from the configured
// endpoint, download and install it, then restart the app.

use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

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

/// Checks for an update. If one is available, downloads and installs it,
/// then restarts the application.
///
/// Only returns to the frontend when no update is found — a successful
/// install calls `app.restart()` which never returns.
#[tauri::command]
pub async fn check_and_install_update(app: AppHandle) -> Result<UpdateInfo, String> {
    let current = app.package_info().version.to_string();

    let updater = app
        .updater_builder()
        .build()
        .map_err(|e| format!("Failed to build updater: {e}"))?;

    let maybe_update = updater
        .check()
        .await
        .map_err(|e| format!("Update check failed: {e}"))?;

    if let Some(update) = maybe_update {
        // Download and install — blocks until the installer finishes.
        update
            .download_and_install(|_chunk, _total| {}, || {})
            .await
            .map_err(|e| format!("Update installation failed: {e}"))?;

        // Restart into the new version; this call never returns.
        app.restart();
    }

    // Reached only when no update is available.
    Ok(UpdateInfo {
        available: false,
        current_version: current,
        latest_version: None,
        release_notes: None,
    })
}
