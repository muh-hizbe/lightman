// Tauri IPC commands for importing a Collection from a JSON file.
//
// Task 11.4: Import command — opens a Tauri file-open dialog, reads the
//            selected file, and dispatches to the correct ImportService method
//            based on the detected format (Postman v2.1 or Lightman JSON).
//
// Requirements: 6.4, 8.3

use crate::models::Collection;
use crate::services::collection as collection_svc;
use crate::services::import::ImportService;
use crate::storage::DbHandle;
use tauri::State;
use tauri_plugin_dialog::DialogExt;

// ---------------------------------------------------------------------------
// import_collection command
// ---------------------------------------------------------------------------

/// Import a collection from a JSON file chosen via the native file-open dialog.
///
/// Supported formats (auto-detected from JSON content):
/// - **Postman Collection v2.1** — detected by the presence of an `info.schema`
///   field containing `"v2.1"`.
/// - **Lightman JSON v2.0** — detected by the presence of a `lightman_version` field.
///
/// Returns `null`/`None` when the user cancels the dialog.
///
/// # Frontend call
/// ```ts
/// invoke<Collection | null>('import_collection')
/// ```
#[tauri::command]
pub async fn import_collection(
    db: State<'_, DbHandle>,
    app: tauri::AppHandle,
) -> Result<Option<Collection>, String> {
    let open_path = app
        .dialog()
        .file()
        .set_title("Import Collection")
        .add_filter("JSON files", &["json"])
        .blocking_pick_file();

    let path = match open_path {
        Some(p) => p,
        None => return Ok(None),
    };

    let path_str = path
        .as_path()
        .ok_or_else(|| "Could not resolve file path to a filesystem path.".to_string())?
        .to_string_lossy()
        .to_string();

    let json = std::fs::read_to_string(&path_str)
        .map_err(|e| format!("Failed to read file '{path_str}': {e}"))?;

    let collection_id = detect_and_import(db.inner().clone(), &json).await?;

    let collection = collection_svc::get_collection(db.inner().clone(), collection_id.clone())
        .await?
        .ok_or_else(|| {
            format!(
                "Import succeeded (id={collection_id}) but the collection could not be found \
                 immediately after insertion."
            )
        })?;

    Ok(Some(collection))
}

// ---------------------------------------------------------------------------
// Format detection helper
// ---------------------------------------------------------------------------

async fn detect_and_import(db: crate::storage::DbHandle, json: &str) -> Result<String, String> {
    let value: serde_json::Value = serde_json::from_str(json)
        .map_err(|e| format!("Failed to parse JSON: {e}"))?;

    let obj = value
        .as_object()
        .ok_or_else(|| "Expected a JSON object at the top level.".to_string())?;

    if obj.contains_key("lightman_version") {
        ImportService::import_lightman(db, json).await
    } else if obj.contains_key("info") && obj["info"].get("schema").is_some() {
        ImportService::import_postman_v21(db, json).await
    } else {
        Err(
            "Unrecognised file format. Expected a Postman Collection v2.1 JSON \
             (with an 'info.schema' field) or a Lightman export JSON \
             (with a 'lightman_version' field)."
                .to_string(),
        )
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AuthConfig, HttpMethod, RequestBody};
    use crate::services::collection as collection_svc;
    use crate::services::export::ExportService;
    use crate::storage;
    use uuid::Uuid;

    fn test_db() -> (crate::storage::DbHandle, std::path::PathBuf) {
        let path = std::env::temp_dir()
            .join(format!("lightman_cmd_import_{}.db", Uuid::new_v4()));
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

    // -----------------------------------------------------------------------
    // detect_and_import: Lightman JSON
    // -----------------------------------------------------------------------

    #[test]
    fn detect_import_lightman_json_creates_collection_and_requests() {
        let (db, _) = test_db();
        rt().block_on(async {
            let col = collection_svc::create_collection(
                db.clone(), "My API".to_string(), None, None, AuthConfig::None,
            )
            .await
            .unwrap();

            for i in 0..2_u8 {
                collection_svc::create_request(
                    db.clone(), col.id.clone(), format!("Request {i}"),
                    HttpMethod::Get, format!("https://api.example.com/{i}"),
                    vec![], vec![], RequestBody::None, AuthConfig::None,
                )
                .await
                .unwrap();
            }

            let export = ExportService::export_collection(db.clone(), col.id.clone())
                .await
                .unwrap();
            let json = ExportService::to_json(&export).unwrap();

            let imported_id = detect_and_import(db.clone(), &json).await.unwrap();

            assert_ne!(imported_id, col.id);

            let imported = collection_svc::get_collection(db.clone(), imported_id.clone())
                .await
                .unwrap()
                .expect("imported collection should exist");
            assert_eq!(imported.name, "My API");

            let reqs = collection_svc::list_requests(db, imported_id).await.unwrap();
            assert_eq!(reqs.len(), 2);
        });
    }

    // -----------------------------------------------------------------------
    // detect_and_import: Postman v2.1
    // -----------------------------------------------------------------------

    #[test]
    fn detect_import_postman_json_creates_collection() {
        let (db, _) = test_db();
        let postman_json = serde_json::json!({
            "info": {
                "name": "Postman API",
                "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
            },
            "item": [
                {
                    "name": "Health Check",
                    "request": { "method": "GET", "url": "https://api.example.com/health" }
                }
            ]
        })
        .to_string();

        rt().block_on(async {
            let col_id = detect_and_import(db.clone(), &postman_json).await.unwrap();

            let col = collection_svc::get_collection(db.clone(), col_id.clone())
                .await
                .unwrap()
                .expect("collection should exist");
            assert_eq!(col.name, "Postman API");

            let reqs = collection_svc::list_requests(db, col_id).await.unwrap();
            assert_eq!(reqs.len(), 1);
            assert_eq!(reqs[0].name, "Health Check");
        });
    }

    // -----------------------------------------------------------------------
    // detect_and_import: unknown format
    // -----------------------------------------------------------------------

    #[test]
    fn detect_import_unknown_format_returns_error() {
        let (db, _) = test_db();
        let unknown_json = serde_json::json!({ "some_other_key": "value", "data": [] }).to_string();
        rt().block_on(async {
            let result = detect_and_import(db, &unknown_json).await;
            assert!(result.is_err());
            assert!(result.unwrap_err().contains("Unrecognised file format"));
        });
    }

    #[test]
    fn detect_import_invalid_json_returns_error() {
        let (db, _) = test_db();
        rt().block_on(async {
            let result = detect_and_import(db, "this is not json").await;
            assert!(result.is_err());
        });
    }

    #[test]
    fn detect_import_array_at_root_returns_error() {
        let (db, _) = test_db();
        rt().block_on(async {
            let result = detect_and_import(db, "[1, 2, 3]").await;
            assert!(result.is_err());
            assert!(result.unwrap_err().contains("JSON object"));
        });
    }

    // -----------------------------------------------------------------------
    // Full round-trip via export string
    // -----------------------------------------------------------------------

    #[test]
    fn roundtrip_export_write_read_import() {
        let (db, _) = test_db();
        rt().block_on(async {
            let col = collection_svc::create_collection(
                db.clone(), "Round Trip".to_string(), None, None, AuthConfig::None,
            )
            .await
            .unwrap();

            collection_svc::create_request(
                db.clone(),
                col.id.clone(),
                "POST request".to_string(),
                HttpMethod::Post,
                "https://api.example.com/items".to_string(),
                vec![crate::models::Header {
                    key: "Accept".to_string(),
                    value: "application/json".to_string(),
                    enabled: true,
                }],
                vec![],
                RequestBody::Json { content: r#"{"name":"test"}"#.to_string() },
                AuthConfig::BearerToken { token: "tok_abc".to_string() },
            )
            .await
            .unwrap();

            let export = ExportService::export_collection(db.clone(), col.id.clone())
                .await
                .unwrap();
            let json = ExportService::to_json(&export).unwrap();

            // Write to temp file, read back (simulates command behavior).
            let tmp_path = std::env::temp_dir()
                .join(format!("lightman_roundtrip_{}.json", Uuid::new_v4()));
            std::fs::write(&tmp_path, json.as_bytes()).unwrap();
            let read_back = std::fs::read_to_string(&tmp_path).unwrap();
            let _ = std::fs::remove_file(&tmp_path);

            let imported_id = detect_and_import(db.clone(), &read_back).await.unwrap();

            assert_ne!(imported_id, col.id);

            let imported = collection_svc::get_collection(db.clone(), imported_id.clone())
                .await
                .unwrap()
                .expect("imported collection should exist");
            assert_eq!(imported.name, "Round Trip");

            let reqs = collection_svc::list_requests(db, imported_id).await.unwrap();
            assert_eq!(reqs.len(), 1);
            assert_eq!(reqs[0].name, "POST request");
        });
    }
}
