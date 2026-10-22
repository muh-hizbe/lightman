// Integration test: export to file then import back — verifies round-trip.
//
// Task 23.2 — Requirements: 6.1, 6.3, 6.4
//
// Tests exercise the full export-import pipeline at the file I/O level,
// mirroring exactly what `commands/export.rs` and `commands/import.rs` do
// without the Tauri dialog wrapper.
//
// Format version: Lightman JSON v2.0

use std::path::PathBuf;
use uuid::Uuid;

use lightman_lib::{
    models::{AuthConfig, Header, HttpMethod, QueryParam, RequestBody},
    services::{
        collection as collection_svc,
        export::ExportService,
        import::ImportService,
    },
    storage,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn test_db() -> (storage::DbHandle, PathBuf) {
    let path = std::env::temp_dir()
        .join(format!("lightman_export_import_{}.db", Uuid::new_v4()));
    let handle = storage::initialize_database(path.clone())
        .expect("initialize_database failed");
    (handle, path)
}

fn write_temp_json(json: &str) -> PathBuf {
    let path = std::env::temp_dir()
        .join(format!("lightman_export_{}.json", Uuid::new_v4()));
    std::fs::write(&path, json.as_bytes()).expect("write temp JSON");
    path
}

// ===========================================================================
// Test 1 — Basic round-trip: empty collection
// ===========================================================================

#[tokio::test]
async fn test_empty_collection_round_trip() {
    let (src_db, _src_path) = test_db();
    let (dst_db, _dst_path) = test_db();

    let col = collection_svc::create_collection(
        src_db.clone(), "Empty Export".to_string(), None, None, AuthConfig::None,
    )
    .await
    .unwrap();

    let export = ExportService::export_collection(src_db.clone(), col.id.clone())
        .await
        .unwrap();
    let json = ExportService::to_json(&export).unwrap();
    let tmp = write_temp_json(&json);

    let read_back = std::fs::read_to_string(&tmp).expect("read temp file");
    let _ = std::fs::remove_file(&tmp);

    let imported_id = ImportService::import_lightman(dst_db.clone(), &read_back)
        .await
        .unwrap();

    let imported = collection_svc::get_collection(dst_db.clone(), imported_id.clone())
        .await
        .unwrap()
        .expect("imported collection must exist");

    assert_eq!(imported.name, "Empty Export");

    let requests = collection_svc::list_requests(dst_db.clone(), imported_id)
        .await
        .unwrap();
    assert!(requests.is_empty(), "imported empty collection should have no requests");
}

// ===========================================================================
// Test 2 — Round-trip: collection with multiple requests
// ===========================================================================

#[tokio::test]
async fn test_collection_with_requests_round_trip() {
    let (src_db, _src_path) = test_db();
    let (dst_db, _dst_path) = test_db();

    let col = collection_svc::create_collection(
        src_db.clone(), "My API".to_string(), None, None, AuthConfig::None,
    )
    .await
    .unwrap();

    let entries = vec![
        ("Get Users",    HttpMethod::Get,    "https://api.example.com/users"),
        ("Create User",  HttpMethod::Post,   "https://api.example.com/users"),
        ("Delete User",  HttpMethod::Delete, "https://api.example.com/users/1"),
    ];

    for (name, method, url) in &entries {
        collection_svc::create_request(
            src_db.clone(), col.id.clone(), name.to_string(), method.clone(),
            url.to_string(), vec![], vec![], RequestBody::None, AuthConfig::None,
        )
        .await
        .unwrap();
    }

    let export = ExportService::export_collection(src_db.clone(), col.id.clone())
        .await
        .unwrap();
    let json = ExportService::to_json(&export).unwrap();
    let tmp = write_temp_json(&json);

    let read_back = std::fs::read_to_string(&tmp).expect("read temp file");
    let _ = std::fs::remove_file(&tmp);
    let imported_id = ImportService::import_lightman(dst_db.clone(), &read_back)
        .await
        .unwrap();

    let imported_requests = collection_svc::list_requests(dst_db.clone(), imported_id)
        .await
        .unwrap();

    assert_eq!(imported_requests.len(), entries.len(), "request count mismatch");

    for (name, method, url) in &entries {
        let found = imported_requests
            .iter()
            .find(|r| r.name == *name)
            .unwrap_or_else(|| panic!("request '{name}' not found after import"));
        assert_eq!(found.method, *method, "method mismatch for '{name}'");
        assert_eq!(found.url, *url, "URL mismatch for '{name}'");
    }
}

// ===========================================================================
// Test 3 — Round-trip: request with full field set
// ===========================================================================

#[tokio::test]
async fn test_request_full_fields_preserved() {
    let (src_db, _src_path) = test_db();
    let (dst_db, _dst_path) = test_db();

    let col = collection_svc::create_collection(
        src_db.clone(), "Full Field API".to_string(), None, None, AuthConfig::None,
    )
    .await
    .unwrap();

    collection_svc::create_request(
        src_db.clone(),
        col.id.clone(),
        "Rich Request".to_string(),
        HttpMethod::Post,
        "https://api.example.com/items".to_string(),
        vec![
            Header { key: "Accept".to_string(), value: "application/json".to_string(), enabled: true },
            Header { key: "X-Custom".to_string(), value: "custom-value".to_string(), enabled: false },
        ],
        vec![QueryParam { key: "page".to_string(), value: "1".to_string(), enabled: true }],
        RequestBody::Json { content: r#"{"name":"test-item","active":true}"#.to_string() },
        AuthConfig::BearerToken { token: "tok-abc123".to_string() },
    )
    .await
    .unwrap();

    let export = ExportService::export_collection(src_db.clone(), col.id.clone())
        .await
        .unwrap();
    let json = ExportService::to_json(&export).unwrap();
    let tmp = write_temp_json(&json);

    let read_back = std::fs::read_to_string(&tmp).expect("read");
    let _ = std::fs::remove_file(&tmp);

    let imported_id = ImportService::import_lightman(dst_db.clone(), &read_back)
        .await
        .unwrap();

    let requests = collection_svc::list_requests(dst_db.clone(), imported_id)
        .await
        .unwrap();

    assert_eq!(requests.len(), 1);
    let req = &requests[0];

    assert_eq!(req.name, "Rich Request");
    assert_eq!(req.method, HttpMethod::Post);
    assert_eq!(req.url, "https://api.example.com/items");
    assert_eq!(req.headers.len(), 2);

    let accept = req.headers.iter().find(|h| h.key == "Accept").unwrap();
    assert_eq!(accept.value, "application/json");
    assert!(accept.enabled);
    let custom = req.headers.iter().find(|h| h.key == "X-Custom").unwrap();
    assert!(!custom.enabled);

    assert_eq!(req.params.len(), 1);
    assert_eq!(req.params[0].key, "page");

    assert!(matches!(&req.body, RequestBody::Json { content } if content.contains("test-item")));
    assert!(matches!(&req.auth, AuthConfig::BearerToken { token } if token == "tok-abc123"));
}

// ===========================================================================
// Test 4 — Round-trip preserves request ordering by position
// ===========================================================================

#[tokio::test]
async fn test_request_order_preserved_after_round_trip() {
    let (src_db, _src_path) = test_db();
    let (dst_db, _dst_path) = test_db();

    let col = collection_svc::create_collection(
        src_db.clone(), "Ordered API".to_string(), None, None, AuthConfig::None,
    )
    .await
    .unwrap();

    for i in 0..5_u8 {
        collection_svc::create_request(
            src_db.clone(), col.id.clone(), format!("Request {i:02}"),
            HttpMethod::Get, format!("https://api.example.com/{i}"),
            vec![], vec![], RequestBody::None, AuthConfig::None,
        )
        .await
        .unwrap();
    }

    let export = ExportService::export_collection(src_db.clone(), col.id.clone())
        .await
        .unwrap();
    let json = ExportService::to_json(&export).unwrap();
    let tmp = write_temp_json(&json);

    let read_back = std::fs::read_to_string(&tmp).expect("read");
    let _ = std::fs::remove_file(&tmp);

    let imported_id = ImportService::import_lightman(dst_db.clone(), &read_back)
        .await
        .unwrap();

    let requests = collection_svc::list_requests(dst_db.clone(), imported_id)
        .await
        .unwrap();

    assert_eq!(requests.len(), 5);

    let positions: Vec<i64> = requests.iter().map(|r| r.position).collect();
    let mut sorted = positions.clone();
    sorted.sort();
    assert_eq!(positions, sorted, "requests must be ordered by position after import");

    let names: Vec<&str> = requests.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, vec!["Request 00", "Request 01", "Request 02", "Request 03", "Request 04"]);
}

// ===========================================================================
// Test 5 — Import assigns new IDs (no collision with existing data)
// ===========================================================================

#[tokio::test]
async fn test_import_assigns_new_ids() {
    let (db, _db_path) = test_db();

    let col = collection_svc::create_collection(
        db.clone(), "Unique IDs".to_string(), None, None, AuthConfig::None,
    )
    .await
    .unwrap();

    collection_svc::create_request(
        db.clone(), col.id.clone(), "Original".to_string(),
        HttpMethod::Get, "https://api.example.com/original".to_string(),
        vec![], vec![], RequestBody::None, AuthConfig::None,
    )
    .await
    .unwrap();

    let original_requests = collection_svc::list_requests(db.clone(), col.id.clone())
        .await
        .unwrap();
    let original_req_id = original_requests[0].id.clone();

    let export = ExportService::export_collection(db.clone(), col.id.clone())
        .await
        .unwrap();
    let json = ExportService::to_json(&export).unwrap();

    let imported_id = ImportService::import_lightman(db.clone(), &json)
        .await
        .expect("import into same DB should succeed with new IDs");

    assert_ne!(imported_id, col.id, "imported collection must have a new unique ID");

    let imported_requests = collection_svc::list_requests(db.clone(), imported_id)
        .await
        .unwrap();
    assert_eq!(imported_requests.len(), 1);
    assert_ne!(imported_requests[0].id, original_req_id, "imported request must have a new unique ID");
}

// ===========================================================================
// Test 6 — Import with corrupted JSON returns an error
// ===========================================================================

#[tokio::test]
async fn test_import_corrupted_file_returns_error() {
    let (db, _db_path) = test_db();
    let tmp = write_temp_json("{ this is not valid json }}}");

    let read_back = std::fs::read_to_string(&tmp).unwrap();
    let _ = std::fs::remove_file(&tmp);

    let result = ImportService::import_lightman(db.clone(), &read_back).await;
    assert!(result.is_err(), "corrupted JSON should return an error");

    let collections = storage::list_collections(db.clone()).await.unwrap();
    assert!(collections.is_empty(), "DB must be unchanged after a failed import");
}

// ===========================================================================
// Test 7 — Import with wrong lightman_version returns an error
// ===========================================================================

#[tokio::test]
async fn test_import_wrong_version_returns_error() {
    let (db, _db_path) = test_db();

    // Build a valid v2.0-shaped envelope but with wrong version string.
    let bad_version_json = serde_json::json!({
        "lightman_version": "99.0",
        "exported_at": "2024-01-01T00:00:00Z",
        "collection": {
            "collection": {
                "id": Uuid::new_v4().to_string(),
                "name": "Bad Version",
                "description": null,
                "parent_id": null,
                "position": 0,
                "created_at": "2024-01-01T00:00:00Z",
                "updated_at": "2024-01-01T00:00:00Z",
                "default_headers": [],
                "auth": {"type": "none"}
            },
            "requests": [],
            "sub_collections": [],
            "saved_responses": []
        },
        "history": [],
        "environments": []
    })
    .to_string();

    let result = ImportService::import_lightman(db.clone(), &bad_version_json).await;
    assert!(result.is_err(), "wrong version should return an error");

    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("99.0"),
        "error message should mention the bad version, got: {err_msg}"
    );
}

// ===========================================================================
// Test 8 — Multiple imports into the same database are additive
// ===========================================================================

#[tokio::test]
async fn test_multiple_imports_are_additive() {
    let (src_db, _src_path) = test_db();
    let (dst_db, _dst_path) = test_db();

    let col = collection_svc::create_collection(
        src_db.clone(), "Additive".to_string(), None, None, AuthConfig::None,
    )
    .await
    .unwrap();

    let export = ExportService::export_collection(src_db.clone(), col.id.clone())
        .await
        .unwrap();
    let json = ExportService::to_json(&export).unwrap();

    let id1 = ImportService::import_lightman(dst_db.clone(), &json).await.unwrap();
    let id2 = ImportService::import_lightman(dst_db.clone(), &json).await.unwrap();

    assert_ne!(id1, id2, "two imports must produce distinct collection IDs");

    assert!(collection_svc::get_collection(dst_db.clone(), id1).await.unwrap().is_some());
    assert!(collection_svc::get_collection(dst_db.clone(), id2).await.unwrap().is_some());
}

// ===========================================================================
// Test 9 — Export produces valid JSON with all required top-level keys
// ===========================================================================

#[tokio::test]
async fn test_export_file_has_required_keys() {
    let (db, _db_path) = test_db();

    let col = collection_svc::create_collection(
        db.clone(), "Key Check".to_string(), None, None, AuthConfig::None,
    )
    .await
    .unwrap();

    let export = ExportService::export_collection(db.clone(), col.id.clone())
        .await
        .unwrap();
    let json = ExportService::to_json(&export).unwrap();
    let tmp = write_temp_json(&json);

    let file_content = std::fs::read_to_string(&tmp).expect("read");
    let _ = std::fs::remove_file(&tmp);

    let parsed: serde_json::Value =
        serde_json::from_str(&file_content).expect("export file must be valid JSON");

    for key in &["lightman_version", "exported_at", "collection", "history", "environments"] {
        assert!(parsed.get(key).is_some(), "export file is missing required key '{key}'");
    }

    assert!(parsed["history"].is_array(), "history must be a JSON array");
    assert!(parsed["environments"].is_array(), "environments must be a JSON array");
    assert_eq!(
        parsed["lightman_version"].as_str().unwrap(),
        lightman_lib::services::export::LIGHTMAN_FORMAT_VERSION
    );
}

// ===========================================================================
// Test 10 — Sub-collections and their requests survive round-trip
// ===========================================================================

#[tokio::test]
async fn test_sub_collections_round_trip() {
    let (src_db, _src_path) = test_db();
    let (dst_db, _dst_path) = test_db();

    let root = collection_svc::create_collection(
        src_db.clone(), "Root API".to_string(), None, None, AuthConfig::None,
    )
    .await
    .unwrap();

    let folder = collection_svc::create_collection(
        src_db.clone(), "Auth".to_string(), None, Some(root.id.clone()), AuthConfig::None,
    )
    .await
    .unwrap();

    collection_svc::create_request(
        src_db.clone(), root.id.clone(), "Root Request".to_string(),
        HttpMethod::Get, "https://api.example.com/root".to_string(),
        vec![], vec![], RequestBody::None, AuthConfig::None,
    )
    .await
    .unwrap();

    collection_svc::create_request(
        src_db.clone(), folder.id.clone(), "Login".to_string(),
        HttpMethod::Post, "https://api.example.com/auth/login".to_string(),
        vec![], vec![], RequestBody::None, AuthConfig::None,
    )
    .await
    .unwrap();

    let export = ExportService::export_collection(src_db.clone(), root.id.clone())
        .await
        .unwrap();
    let json = ExportService::to_json(&export).unwrap();
    let tmp = write_temp_json(&json);

    let read_back = std::fs::read_to_string(&tmp).expect("read");
    let _ = std::fs::remove_file(&tmp);

    let new_root_id = ImportService::import_lightman(dst_db.clone(), &read_back)
        .await
        .unwrap();

    // Root requests
    let root_reqs = collection_svc::list_requests(dst_db.clone(), new_root_id.clone())
        .await
        .unwrap();
    assert_eq!(root_reqs.len(), 1);
    assert_eq!(root_reqs[0].name, "Root Request");

    // Sub-collection
    let all_cols = collection_svc::list_collections(dst_db.clone()).await.unwrap();
    let new_folder = all_cols
        .iter()
        .find(|c| c.name == "Auth" && c.parent_id.as_deref() == Some(&new_root_id))
        .expect("imported sub-collection 'Auth' should exist");

    let folder_reqs = collection_svc::list_requests(dst_db.clone(), new_folder.id.clone())
        .await
        .unwrap();
    assert_eq!(folder_reqs.len(), 1);
    assert_eq!(folder_reqs[0].name, "Login");
}

// ===========================================================================
// Test 11 — Auth types preserved in round-trip
// ===========================================================================

#[tokio::test]
async fn test_auth_types_preserved_in_round_trip() {
    let (src_db, _src_path) = test_db();
    let (dst_db, _dst_path) = test_db();

    let col = collection_svc::create_collection(
        src_db.clone(), "Auth Types".to_string(), None, None, AuthConfig::None,
    )
    .await
    .unwrap();

    collection_svc::create_request(
        src_db.clone(), col.id.clone(), "Basic Auth Request".to_string(),
        HttpMethod::Get, "https://api.example.com/secure".to_string(),
        vec![], vec![], RequestBody::None,
        AuthConfig::BasicAuth { username: "alice".to_string(), password: "wonderland".to_string() },
    )
    .await
    .unwrap();

    collection_svc::create_request(
        src_db.clone(), col.id.clone(), "ApiKey Request".to_string(),
        HttpMethod::Get, "https://api.example.com/data".to_string(),
        vec![], vec![], RequestBody::None,
        AuthConfig::ApiKey {
            key: "x-api-key".to_string(),
            value: "k3y-s3cr3t".to_string(),
            location: lightman_lib::models::ApiKeyLocation::Header,
        },
    )
    .await
    .unwrap();

    let export = ExportService::export_collection(src_db.clone(), col.id.clone())
        .await
        .unwrap();
    let json = ExportService::to_json(&export).unwrap();
    let tmp = write_temp_json(&json);

    let read_back = std::fs::read_to_string(&tmp).unwrap();
    let _ = std::fs::remove_file(&tmp);

    let imported_id = ImportService::import_lightman(dst_db.clone(), &read_back)
        .await
        .unwrap();

    let requests = collection_svc::list_requests(dst_db.clone(), imported_id)
        .await
        .unwrap();

    assert_eq!(requests.len(), 2);

    let basic = requests.iter().find(|r| r.name == "Basic Auth Request").unwrap();
    assert!(matches!(&basic.auth, AuthConfig::BasicAuth { username, .. } if username == "alice"));

    let api_key = requests.iter().find(|r| r.name == "ApiKey Request").unwrap();
    assert!(matches!(&api_key.auth, AuthConfig::ApiKey { value, .. } if value == "k3y-s3cr3t"));
}
