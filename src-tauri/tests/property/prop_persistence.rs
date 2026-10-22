// Property-based tests untuk storage persistence layer.
//
// Task 2.4 — dua property divalidasi:
//
//   Property 2: Request Config Round-Trip Persistence
//     Untuk kombinasi field request yang arbitrari, insert + read-back harus
//     menghasilkan nilai yang identik.
//     Validates: Requirements 2.3, 7.1
//
//   Property 3: Recursive Delete Completeness
//     Setelah collection dihapus, tidak boleh ada request yang tersisa di DB,
//     berapa pun banyaknya request yang sebelumnya dimasukkan.
//     Validates: Requirement 2.5

use proptest::prelude::*;
use lightman_lib::storage::{
    DbHandle, initialize_database, insert_collection, insert_request,
    get_request, list_requests_by_collection, delete_collection,
};

// ---------------------------------------------------------------------------
// Test-DB helper
// ---------------------------------------------------------------------------

/// Buka fresh file-backed SQLite database di direktori temp.
/// Mengembalikan (DbHandle, path) — caller bertanggung jawab menghapus file
/// setelah handle di-drop.
fn open_test_db() -> (DbHandle, std::path::PathBuf) {
    let tmp_path = std::env::temp_dir()
        .join(format!("lightman_prop_{}.db", uuid::Uuid::new_v4()));
    let handle = initialize_database(tmp_path.clone())
        .expect("initialize_database should succeed");
    (handle, tmp_path)
}

/// Seed collection minimal dan kembalikan ID-nya (sync, di dalam tokio context).
async fn seed_collection(handle: DbHandle, name: &str) -> String {
    let id  = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    insert_collection(
        handle,
        id.clone(),
        name.to_string(),
        None,
        None,
        0,
        r#"{"type":"none"}"#.to_string(),
        now.clone(),
        now,
    )
    .await
    .expect("seed insert_collection should not fail");
    id
}

// ---------------------------------------------------------------------------
// Strategies
// ---------------------------------------------------------------------------

/// Nama valid (1–50 karakter, hanya alphanumeric + spasi/underscore/dash).
fn valid_name() -> impl Strategy<Value = String> {
    proptest::string::string_regex("[A-Za-z0-9 _\\-]{1,50}")
        .expect("valid_name regex should compile")
}

/// HTTP method yang valid.
fn http_method() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("GET".to_string()),
        Just("POST".to_string()),
        Just("PUT".to_string()),
        Just("PATCH".to_string()),
        Just("DELETE".to_string()),
        Just("HEAD".to_string()),
        Just("OPTIONS".to_string()),
    ]
}

/// URL yang plausibel.
fn any_url() -> impl Strategy<Value = String> {
    proptest::string::string_regex(
        "https?://[a-z]{3,10}\\.[a-z]{2,4}(/[a-z0-9]{0,10}){0,3}",
    )
    .expect("any_url regex should compile")
}

/// Position (indeks urutan).
fn position() -> impl Strategy<Value = i64> {
    0i64..1000i64
}

// ---------------------------------------------------------------------------
// Helper: bangun tokio runtime untuk satu blok async
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
// Property 2 — Request Config Round-Trip Persistence
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_request_config_round_trip(
        req_name in valid_name(),
        method   in http_method(),
        url      in any_url(),
        pos      in position(),
    ) {
        let (handle, db_path) = open_test_db();

        // Seed parent collection.
        let col_id = run_async(seed_collection(handle.clone(), "rt-col"));

        let req_id  = uuid::Uuid::new_v4().to_string();
        let now     = chrono::Utc::now().to_rfc3339();

        let headers = "[]".to_string();
        let params  = "[]".to_string();
        let body    = "{\"type\":\"none\"}".to_string();
        let auth    = "{\"type\":\"none\"}".to_string();

        // Insert.
        run_async(insert_request(
            handle.clone(),
            req_id.clone(),
            col_id.clone(),
            req_name.clone(),
            method.clone(),
            url.clone(),
            headers.clone(),
            params.clone(),
            body.clone(),
            auth.clone(),
            pos,
            now.clone(),
            now.clone(),
        ))
        .expect("insert_request should succeed");

        // Read back.
        let row = run_async(get_request(handle.clone(), req_id.clone()))
            .expect("get_request should not return error")
            .expect("row must exist after insert");

        // Cleanup sebelum assertions agar file DB dilepas.
        drop(handle);
        let _ = std::fs::remove_file(&db_path);

        // Verifikasi setiap field.
        prop_assert_eq!(&row.id,            &req_id,   "id round-trip");
        prop_assert_eq!(&row.collection_id, &col_id,   "collection_id round-trip");
        prop_assert_eq!(&row.name,          &req_name, "name round-trip");
        prop_assert_eq!(&row.method,        &method,   "method round-trip");
        prop_assert_eq!(&row.url,           &url,      "url round-trip");
        prop_assert_eq!(&row.headers_json,  &headers,  "headers_json round-trip");
        prop_assert_eq!(&row.params_json,   &params,   "params_json round-trip");
        prop_assert_eq!(&row.body_json,     &body,     "body_json round-trip");
        prop_assert_eq!(&row.auth_json,     &auth,     "auth_json round-trip");
        prop_assert_eq!(row.position,        pos,      "position round-trip");
    }
}

// ---------------------------------------------------------------------------
// Property 3 — Recursive Delete Completeness
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_recursive_delete_removes_all_requests(
        req_count in 0usize..=20usize,
        extra_names in prop::collection::vec(valid_name(), 0..=20),
    ) {
        let (handle, db_path) = open_test_db();

        let col_id     = run_async(seed_collection(handle.clone(), "target"));
        let sibling_id = run_async(seed_collection(handle.clone(), "sibling"));

        let now = chrono::Utc::now().to_rfc3339();
        let mut inserted_ids: Vec<String> = Vec::with_capacity(req_count);

        for i in 0..req_count {
            // Request di target collection.
            let req_id = uuid::Uuid::new_v4().to_string();
            let name = extra_names.get(i)
                .filter(|s| !s.is_empty())
                .cloned()
                .unwrap_or_else(|| format!("req-{i}"));

            run_async(insert_request(
                handle.clone(),
                req_id.clone(),
                col_id.clone(),
                name,
                "GET".to_string(),
                "https://example.com".to_string(),
                "[]".to_string(),
                "[]".to_string(),
                "{\"type\":\"none\"}".to_string(),
                "{\"type\":\"none\"}".to_string(),
                i as i64,
                now.clone(),
                now.clone(),
            ))
            .expect("target insert_request should succeed");
            inserted_ids.push(req_id);

            // Request di sibling — harus tetap ada setelah target dihapus.
            let sib_req_id = uuid::Uuid::new_v4().to_string();
            run_async(insert_request(
                handle.clone(),
                sib_req_id,
                sibling_id.clone(),
                format!("sib-{i}"),
                "GET".to_string(),
                "https://example.com".to_string(),
                "[]".to_string(),
                "[]".to_string(),
                "{\"type\":\"none\"}".to_string(),
                "{\"type\":\"none\"}".to_string(),
                i as i64,
                now.clone(),
                now.clone(),
            ))
            .expect("sibling insert_request should succeed");
        }

        // Pre-condition: semua request target harus ada.
        for req_id in &inserted_ids {
            let row = run_async(get_request(handle.clone(), req_id.clone()))
                .expect("pre-delete get_request should not error");
            prop_assert!(row.is_some(), "request {req_id} must exist before delete");
        }

        // Hapus collection target — cascade ke semua requests-nya.
        run_async(delete_collection(handle.clone(), col_id.clone()))
            .expect("delete_collection should not error");

        // Post-condition 1: list harus kosong untuk collection yang dihapus.
        let remaining = run_async(list_requests_by_collection(handle.clone(), col_id.clone()))
            .expect("list after delete should not error");
        prop_assert_eq!(
            remaining.len(),
            0,
            "no requests should remain after collection delete"
        );

        // Post-condition 2: setiap request individual harus tidak ada lagi.
        for req_id in &inserted_ids {
            let row = run_async(get_request(handle.clone(), req_id.clone()))
                .expect("post-delete get_request should not error");
            prop_assert!(
                row.is_none(),
                "request {req_id} must be gone after collection delete"
            );
        }

        // Post-condition 3: requests sibling tidak terpengaruh.
        if req_count > 0 {
            let sib_remaining =
                run_async(list_requests_by_collection(handle.clone(), sibling_id.clone()))
                    .expect("sibling list should not error");
            prop_assert_eq!(
                sib_remaining.len(),
                req_count,
                "sibling requests must survive deletion of target collection"
            );
        }

        // Cleanup.
        drop(handle);
        let _ = std::fs::remove_file(&db_path);
    }
}
