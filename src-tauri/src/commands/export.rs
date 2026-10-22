// Tauri IPC commands for exporting a Collection to a JSON file.
//
// Task 11.4: Export command — opens a Tauri save dialog to pick a path, then
//            writes the Lightman JSON to disk.
//
// Requirements: 6.4, 8.3

use crate::services::export::ExportService;
use crate::storage::DbHandle;
use tauri::State;
use tauri_plugin_dialog::DialogExt;

// ---------------------------------------------------------------------------
// export_collection command
// ---------------------------------------------------------------------------

/// Export a collection (and all its requests, sub-collections, saved responses,
/// history entries, and environments) to a Lightman JSON v2.0 file.
///
/// Steps:
/// 1. Build the export payload first (fail fast before opening the dialog).
/// 2. Open a native "Save File" dialog filtered to `.json` files.
/// 3. Write the pretty-printed JSON to the chosen path.
///
/// Returns the absolute path that was written, or `null`/`None` when the
/// user cancelled the dialog.
///
/// # Frontend call
/// ```ts
/// invoke<string | null>('export_collection', { collectionId })
/// ```
#[tauri::command]
pub async fn export_collection(
    db: State<'_, DbHandle>,
    app: tauri::AppHandle,
    collection_id: String,
) -> Result<Option<String>, String> {
    // -----------------------------------------------------------------------
    // 1. Build the export payload first (fail fast before opening the dialog).
    // -----------------------------------------------------------------------
    let export = ExportService::export_collection(db.inner().clone(), collection_id).await?;
    let json = ExportService::to_json(&export)?;

    // -----------------------------------------------------------------------
    // 2. Open the native "Save File" dialog.
    // -----------------------------------------------------------------------
    let save_path = app
        .dialog()
        .file()
        .set_title("Export Collection")
        .set_file_name(format!("{}.json", export.collection.collection.name))
        .add_filter("JSON files", &["json"])
        .blocking_save_file();

    let path = match save_path {
        Some(p) => p,
        // User cancelled — return null to the frontend.
        None => return Ok(None),
    };

    // -----------------------------------------------------------------------
    // 3. Write the JSON to the chosen path.
    // -----------------------------------------------------------------------
    let path_str = path
        .as_path()
        .ok_or_else(|| "Could not resolve save path to a filesystem path.".to_string())?
        .to_string_lossy()
        .to_string();

    std::fs::write(&path_str, json.as_bytes())
        .map_err(|e| format!("Failed to write export file '{path_str}': {e}"))?;

    Ok(Some(path_str))
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::models::{AuthConfig, HttpMethod, RequestBody};
    use crate::services::collection as collection_svc;
    use crate::services::export::ExportService;
    use crate::storage;
    use uuid::Uuid;

    fn test_db() -> (crate::storage::DbHandle, std::path::PathBuf) {
        let path = std::env::temp_dir()
            .join(format!("lightman_cmd_export_{}.db", Uuid::new_v4()));
        let handle = storage::initialize_database(path.clone())
            .expect("initialize_database");
        (handle, path)
    }

    fn rt() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
    }

    #[test]
    fn export_produces_valid_json_that_can_be_written_to_disk() {
        let (db, _) = test_db();
        rt().block_on(async {
            let col = collection_svc::create_collection(
                db.clone(),
                "My API".to_string(),
                None,
                None,
                AuthConfig::None,
            )
            .await
            .unwrap();

            collection_svc::create_request(
                db.clone(),
                col.id.clone(),
                "Get Data".to_string(),
                HttpMethod::Get,
                "https://api.example.com/data".to_string(),
                vec![],
                vec![],
                RequestBody::None,
                AuthConfig::None,
            )
            .await
            .unwrap();

            let export = ExportService::export_collection(db, col.id.clone()).await.unwrap();
            let json = ExportService::to_json(&export).unwrap();

            // Must be valid JSON with the v2.0 shape.
            let parsed: serde_json::Value =
                serde_json::from_str(&json).expect("export produced invalid JSON");

            assert_eq!(
                parsed["lightman_version"].as_str().unwrap(),
                crate::services::export::LIGHTMAN_FORMAT_VERSION
            );
            // collection is now a CollectionNode, not bare Collection
            assert_eq!(
                parsed["collection"]["collection"]["id"].as_str().unwrap(),
                col.id
            );
            assert!(parsed["history"].is_array());
            assert!(parsed["environments"].is_array());

            // Simulate writing to disk.
            let tmp_path = std::env::temp_dir()
                .join(format!("lightman_export_test_{}.json", Uuid::new_v4()));
            std::fs::write(&tmp_path, json.as_bytes()).expect("write failed");

            let content = std::fs::read_to_string(&tmp_path).expect("read failed");
            let reparsed: serde_json::Value = serde_json::from_str(&content).unwrap();
            assert_eq!(
                reparsed["collection"]["collection"]["name"].as_str().unwrap(),
                "My API"
            );

            let _ = std::fs::remove_file(tmp_path);
        });
    }

    #[test]
    fn export_nonexistent_collection_returns_error_before_dialog() {
        let (db, _) = test_db();
        rt().block_on(async {
            let result =
                ExportService::export_collection(db, "no-such-id".to_string()).await;
            assert!(result.is_err());
            assert!(result.unwrap_err().contains("not found"));
        });
    }
}
