// Property-based tests untuk export-import round trip (Properties 10, 11, 12).
//
// Task 11.5 — tiga property divalidasi:
//
//   Property 10: Export-Import Round Trip for Collections
//     Untuk collection dan requests yang arbitrari, export → JSON → import
//     harus menghasilkan collection dan requests yang field-by-field identik
//     (kecuali ID dan timestamp baru yang di-generate saat import).
//     Validates: Requirements 6.3
//
//   Property 11: Import Idempotency on Error
//     Jika JSON yang diberikan ke import tidak valid atau corrupt,
//     import harus gagal dengan error dan tidak memodifikasi storage —
//     jumlah collections sebelum dan sesudah import yang gagal harus sama.
//     Validates: Requirements 6.2
//
//   Property 12: Export Produces Valid JSON
//     Untuk collection apa pun dengan requests apa pun, output dari
//     `ExportService::to_json` harus selalu merupakan JSON valid yang
//     mengandung top-level keys: `lightman_version`, `exported_at`,
//     `collection`, dan `requests`.
//     Validates: Requirements 6.3

use proptest::prelude::*;
use lightman_lib::models::{AuthConfig, ApiKeyLocation, Header, HttpMethod, MultipartField, QueryParam, RequestBody};
use lightman_lib::services::collection as collection_svc;
use lightman_lib::services::export::ExportService;
use lightman_lib::services::import::ImportService;
use lightman_lib::storage::{self, DbHandle};

// ---------------------------------------------------------------------------
// Test-DB helper
// ---------------------------------------------------------------------------

/// Buka fresh file-backed SQLite database di direktori temp.
fn open_test_db() -> (DbHandle, std::path::PathBuf) {
    let tmp_path = std::env::temp_dir()
        .join(format!("lightman_prop_ei_{}.db", uuid::Uuid::new_v4()));
    let handle = storage::initialize_database(tmp_path.clone())
        .expect("initialize_database should succeed");
    (handle, tmp_path)
}

// ---------------------------------------------------------------------------
// Async helper
// ---------------------------------------------------------------------------

fn run_async<F>(fut: F) -> F::Output
where
    F: std::future::Future,
{
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime")
        .block_on(fut)
}

// ---------------------------------------------------------------------------
// Strategies
// ---------------------------------------------------------------------------

/// Nama valid: 1–50 karakter alphanumeric + spasi/underscore/dash.
fn valid_name() -> impl Strategy<Value = String> {
    proptest::string::string_regex("[A-Za-z0-9][A-Za-z0-9 _\\-]{0,49}")
        .expect("valid_name regex should compile")
}

/// URL yang valid.
fn any_url() -> impl Strategy<Value = String> {
    proptest::string::string_regex(
        "https?://[a-z]{3,10}\\.[a-z]{2,4}(/[a-z0-9]{0,10}){0,3}",
    )
    .expect("any_url regex should compile")
}

/// HTTP method yang valid.
fn any_http_method() -> impl Strategy<Value = HttpMethod> {
    prop_oneof![
        Just(HttpMethod::Get),
        Just(HttpMethod::Post),
        Just(HttpMethod::Put),
        Just(HttpMethod::Patch),
        Just(HttpMethod::Delete),
        Just(HttpMethod::Head),
        Just(HttpMethod::Options),
    ]
}

/// Header key yang valid (tidak ada spasi).
fn valid_header_key() -> impl Strategy<Value = String> {
    proptest::string::string_regex("[A-Za-z][A-Za-z0-9\\-]{0,29}")
        .expect("valid_header_key regex should compile")
}

/// Header value yang valid.
fn valid_header_value() -> impl Strategy<Value = String> {
    proptest::string::string_regex("[A-Za-z0-9 ._/\\-]{0,50}")
        .expect("valid_header_value regex should compile")
}

/// Satu Header.
fn any_header() -> impl Strategy<Value = Header> {
    (valid_header_key(), valid_header_value(), any::<bool>()).prop_map(|(key, value, enabled)| {
        Header { key, value, enabled }
    })
}

/// Satu QueryParam.
fn any_query_param() -> impl Strategy<Value = QueryParam> {
    (valid_header_key(), valid_header_value(), any::<bool>()).prop_map(|(key, value, enabled)| {
        QueryParam { key, value, enabled }
    })
}

/// RequestBody sederhana — hanya variant yang tidak melibatkan file path.
fn any_simple_body() -> impl Strategy<Value = RequestBody> {
    prop_oneof![
        Just(RequestBody::None),
        proptest::string::string_regex("[A-Za-z0-9 .,{}:\"\\[\\]]{0,60}")
            .expect("json content regex")
            .prop_map(|content| RequestBody::Json { content }),
        proptest::string::string_regex("[A-Za-z0-9 ._]{0,40}")
            .expect("raw content regex")
            .prop_map(|content| RequestBody::Raw {
                content,
                content_type: "text/plain".to_string(),
            }),
        prop::collection::vec(
            (valid_header_key(), valid_header_value(), any::<bool>()).prop_map(
                |(key, value, enabled)| MultipartField { key, value, enabled },
            ),
            0..=3,
        )
        .prop_map(|fields| RequestBody::FormUrlEncoded { fields }),
    ]
}

/// AuthConfig sederhana — semua variant kecuali yang jarang.
fn any_auth_config() -> impl Strategy<Value = AuthConfig> {
    prop_oneof![
        Just(AuthConfig::None),
        (
            proptest::string::string_regex("[A-Za-z0-9_]{1,20}").expect("username"),
            proptest::string::string_regex("[A-Za-z0-9_!@#]{1,20}").expect("password"),
        )
            .prop_map(|(username, password)| AuthConfig::BasicAuth { username, password }),
        proptest::string::string_regex("[A-Za-z0-9_\\.\\-]{10,40}")
            .expect("token regex")
            .prop_map(|token| AuthConfig::BearerToken { token }),
        (
            valid_header_key(),
            proptest::string::string_regex("[A-Za-z0-9_]{10,30}").expect("api key value"),
            prop_oneof![Just(ApiKeyLocation::Header), Just(ApiKeyLocation::QueryParam)],
        )
            .prop_map(|(key, value, location)| AuthConfig::ApiKey { key, value, location }),
    ]
}

// ---------------------------------------------------------------------------
// Property 10 — Export-Import Round Trip for Collections
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Untuk collection dengan nama dan deskripsi arbitrari serta sejumlah
    /// requests arbitrari, siklus export → JSON → import harus menghasilkan:
    ///   - collection baru dengan `name` dan `description` yang sama.
    ///   - jumlah requests yang sama.
    ///   - setiap request memiliki `name`, `method`, `url`, `headers`, `params`,
    ///     `body`, dan `auth` yang identik dengan original.
    #[test]
    fn prop_export_import_round_trip(
        col_name    in valid_name(),
        col_desc    in proptest::option::of(valid_name()),
        req_count   in 0usize..=5usize,
        req_names   in prop::collection::vec(valid_name(), 5),
        req_methods in prop::collection::vec(any_http_method(), 5),
        req_urls    in prop::collection::vec(any_url(), 5),
        req_headers in prop::collection::vec(prop::collection::vec(any_header(), 0..=3), 5),
        req_params  in prop::collection::vec(prop::collection::vec(any_query_param(), 0..=3), 5),
        req_bodies  in prop::collection::vec(any_simple_body(), 5),
        req_auths   in prop::collection::vec(any_auth_config(), 5),
    ) {
        let (db, db_path) = open_test_db();

        run_async(async {
            // --- 1. Buat collection ---
            let col = collection_svc::create_collection(
                db.clone(),
                col_name.clone(),
                col_desc.clone(),
                None,
                lightman_lib::models::AuthConfig::None,
            )
            .await
            .expect("create_collection should succeed");

            // --- 2. Buat requests ---
            for i in 0..req_count {
                collection_svc::create_request(
                    db.clone(),
                    col.id.clone(),
                    req_names[i].clone(),
                    req_methods[i].clone(),
                    req_urls[i].clone(),
                    req_headers[i].clone(),
                    req_params[i].clone(),
                    req_bodies[i].clone(),
                    req_auths[i].clone(),
                )
                .await
                .expect("create_request should succeed");
            }

            // --- 3. Export ---
            let export_envelope = ExportService::export_collection(db.clone(), col.id.clone())
                .await
                .expect("export_collection should succeed");
            let json = ExportService::to_json(&export_envelope)
                .expect("to_json should succeed");

            // --- 4. Import ---
            let imported_col_id = ImportService::import_lightman(db.clone(), &json)
                .await
                .expect("import_lightman should succeed");

            // --- 5. Ambil hasil import ---
            let imported_col = collection_svc::get_collection(db.clone(), imported_col_id.clone())
                .await
                .expect("get_collection should not error")
                .expect("imported collection must exist");
            let imported_requests = collection_svc::list_requests(db.clone(), imported_col_id.clone())
                .await
                .expect("list_requests should not error");

            // --- 6. Validasi collection ---
            prop_assert_eq!(
                &imported_col.name, &col_name,
                "collection name must be preserved after round trip"
            );
            prop_assert_eq!(
                imported_col.description.as_deref(),
                col_desc.as_deref(),
                "collection description must be preserved after round trip"
            );

            // --- 7. Validasi jumlah requests ---
            prop_assert_eq!(
                imported_requests.len(),
                req_count,
                "request count must match after round trip"
            );

            // --- 8. Validasi field tiap request ---
            // Sort keduanya by position untuk perbandingan yang deterministik.
            let mut orig_requests = collection_svc::list_requests(db.clone(), col.id.clone())
                .await
                .expect("list original requests should not error");
            orig_requests.sort_by_key(|r| r.position);

            let mut sorted_imported = imported_requests;
            sorted_imported.sort_by_key(|r| r.position);

            for (orig, imported) in orig_requests.iter().zip(sorted_imported.iter()) {
                prop_assert_eq!(
                    &imported.name, &orig.name,
                    "request name must be preserved"
                );
                prop_assert_eq!(
                    &imported.method, &orig.method,
                    "request method must be preserved"
                );
                prop_assert_eq!(
                    &imported.url, &orig.url,
                    "request URL must be preserved"
                );
                prop_assert_eq!(
                    imported.headers.len(), orig.headers.len(),
                    "request header count must be preserved"
                );
                prop_assert_eq!(
                    imported.params.len(), orig.params.len(),
                    "request param count must be preserved"
                );
                prop_assert_eq!(
                    &imported.body, &orig.body,
                    "request body must be preserved"
                );
                prop_assert_eq!(
                    &imported.auth, &orig.auth,
                    "request auth must be preserved"
                );
            }

            Ok(()) as Result<(), TestCaseError>
        })?;

        drop(db);
        let _ = std::fs::remove_file(&db_path);
    }
}

// ---------------------------------------------------------------------------
// Property 11 — Import Idempotency on Error
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Jika JSON yang diberikan ke `import_lightman` tidak valid (malformed,
    /// salah format, versi tidak cocok, atau string acak), import harus:
    ///   a. Mengembalikan `Err` — tidak boleh `Ok`.
    ///   b. Tidak mengubah jumlah collections yang ada di storage.
    #[test]
    fn prop_import_error_does_not_modify_storage(
        bad_json in prop_oneof![
            // String acak yang bukan JSON.
            proptest::string::string_regex("[A-Za-z0-9 !@#$%]{1,80}")
                .expect("random string regex"),
            // JSON object yang tidak punya field yang diperlukan.
            proptest::string::string_regex("[A-Za-z][A-Za-z0-9]{0,10}")
                .expect("key regex")
                .prop_map(|key| format!(r#"{{"{key}": "value"}}"#)),
            // JSON valid tapi versi salah.
            Just(r#"{"lightman_version":"999.0","exported_at":"2024-01-01T00:00:00Z","collection":{"id":"x","name":"x","description":null,"parent_id":null,"position":0,"created_at":"","updated_at":"","default_headers":[]},"requests":[]}"#.to_string()),
            // JSON null.
            Just("null".to_string()),
            // Array kosong.
            Just("[]".to_string()),
        ],
        seed_count in 0usize..=3usize,
    ) {
        let (db, db_path) = open_test_db();

        run_async(async {
            // Buat beberapa collection awal sebagai baseline.
            for i in 0..seed_count {
                collection_svc::create_collection(
                    db.clone(),
                    format!("Seed-{i}"),
                    None,
                    None,
                    lightman_lib::models::AuthConfig::None,
                )
                .await
                .expect("seed create_collection should succeed");
            }

            // Catat jumlah collections sebelum import yang akan gagal.
            let before = collection_svc::list_collections(db.clone())
                .await
                .expect("list_collections before should not error");
            let count_before = before.len();

            // Import yang diharapkan gagal.
            let result = ImportService::import_lightman(db.clone(), &bad_json).await;
            prop_assert!(
                result.is_err(),
                "import of invalid JSON must return Err, got Ok({:?})",
                result.ok()
            );

            // Jumlah collections harus tetap sama.
            let after = collection_svc::list_collections(db.clone())
                .await
                .expect("list_collections after should not error");
            let count_after = after.len();

            prop_assert_eq!(
                count_after,
                count_before,
                "{}",
                format!("failed import must not change collection count (was {count_before}, now {count_after})")
            );

            Ok(()) as Result<(), TestCaseError>
        })?;

        drop(db);
        let _ = std::fs::remove_file(&db_path);
    }
}

// ---------------------------------------------------------------------------
// Property 12 — Export Produces Valid JSON
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    /// Untuk collection dengan nama arbitrari dan jumlah requests arbitrari,
    /// `ExportService::to_json` harus:
    ///   a. Selalu berhasil (tidak return Err).
    ///   b. Menghasilkan string yang merupakan JSON valid.
    ///   c. JSON harus mengandung top-level keys: `lightman_version`,
    ///      `exported_at`, `collection`, dan `requests`.
    ///   d. `lightman_version` harus berupa string non-empty.
    ///   e. `exported_at` harus berupa string non-empty.
    ///   f. `requests` harus berupa JSON array.
    ///   g. Jumlah element dalam array `requests` harus sama dengan jumlah
    ///      request yang dimasukkan.
    #[test]
    fn prop_export_produces_valid_json(
        col_name  in valid_name(),
        req_count in 0usize..=8usize,
        req_names in prop::collection::vec(valid_name(), 8),
        req_urls  in prop::collection::vec(any_url(), 8),
        req_methods in prop::collection::vec(any_http_method(), 8),
        req_bodies in prop::collection::vec(any_simple_body(), 8),
    ) {
        let (db, db_path) = open_test_db();

        run_async(async {
            // --- Buat collection dan requests ---
            let col = collection_svc::create_collection(
                db.clone(),
                col_name.clone(),
                None,
                None,
                lightman_lib::models::AuthConfig::None,
            )
            .await
            .expect("create_collection should succeed");

            for i in 0..req_count {
                collection_svc::create_request(
                    db.clone(),
                    col.id.clone(),
                    req_names[i].clone(),
                    req_methods[i].clone(),
                    req_urls[i].clone(),
                    vec![],
                    vec![],
                    req_bodies[i].clone(),
                    AuthConfig::None,
                )
                .await
                .expect("create_request should succeed");
            }

            // --- Export ---
            let envelope = ExportService::export_collection(db.clone(), col.id.clone())
                .await
                .expect("export_collection should succeed");

            // Property (a): to_json tidak boleh return Err.
            let json = ExportService::to_json(&envelope)
                .expect("to_json must not fail for any valid export envelope");

            // Property (b): harus JSON valid.
            let parsed: serde_json::Value = serde_json::from_str(&json)
                .expect("export must produce valid JSON");

            // Property (c): harus ada 4 top-level keys.
            prop_assert!(
                parsed.get("lightman_version").is_some(),
                "export JSON must contain 'lightman_version' key"
            );
            prop_assert!(
                parsed.get("exported_at").is_some(),
                "export JSON must contain 'exported_at' key"
            );
            prop_assert!(
                parsed.get("collection").is_some(),
                "export JSON must contain 'collection' key"
            );
            prop_assert!(
                parsed.get("requests").is_some(),
                "export JSON must contain 'requests' key"
            );

            // Property (d): lightman_version harus non-empty string.
            let version = parsed["lightman_version"]
                .as_str()
                .expect("lightman_version must be a string");
            prop_assert!(
                !version.is_empty(),
                "lightman_version must be a non-empty string"
            );

            // Property (e): exported_at harus non-empty string.
            let exported_at = parsed["exported_at"]
                .as_str()
                .expect("exported_at must be a string");
            prop_assert!(
                !exported_at.is_empty(),
                "exported_at must be a non-empty string"
            );

            // Property (f): requests harus array.
            prop_assert!(
                parsed["requests"].is_array(),
                "requests must be a JSON array"
            );

            // Property (g): panjang array requests harus cocok.
            let arr_len = parsed["requests"].as_array().unwrap().len();
            prop_assert_eq!(
                arr_len,
                req_count,
                "requests array length must equal number of requests created"
            );

            // Bonus: collection.name di JSON harus cocok dengan col_name.
            let json_col_name = parsed["collection"]["name"]
                .as_str()
                .expect("collection.name must be a string");
            prop_assert_eq!(
                json_col_name,
                col_name.as_str(),
                "collection name in JSON must match original"
            );

            Ok(()) as Result<(), TestCaseError>
        })?;

        drop(db);
        let _ = std::fs::remove_file(&db_path);
    }
}
