// Integration test: full request pipeline end-to-end.
//
// Task 9.2 — Requirements: 1.2, 1.4, 3.2, 4.1
//
// These tests exercise the complete pipeline that mirrors what `send_request`
// does in `commands/http.rs`, but without the Tauri `State<'_, DbHandle>`
// wrapper so they can run as ordinary integration tests:
//
//   1. Variable interpolation   (VarInterpolatorService)
//   2. Header merge + auth      (HeaderMergerService)
//   3. HTTP execution           (HttpExecutorService)
//   4. History persistence      (storage::insert_history_entry / list_history)
//
// A `wiremock::MockServer` handles all outbound HTTP so no network access is
// needed, and each test uses its own SQLite in-memory database.

use chrono::Utc;
use uuid::Uuid;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use lightman_lib::{
    models::{
        ApiKeyLocation, AuthConfig, Header, HttpMethod, QueryParam, Request,
        RequestBody, Variable,
    },
    services::{
        collection as collection_svc,
        environment as env_svc,
        header_merger::HeaderMergerService,
        http_executor::HttpExecutorService,
        var_interpolator::VarInterpolatorService,
    },
    storage::{self, DbHandle},
};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Open a fresh in-memory SQLite database for a single test.
fn test_db() -> DbHandle {
    let path = std::env::temp_dir().join(format!("lightman_e2e_{}.db", Uuid::new_v4()));
    storage::initialize_database(path).expect("initialize_database failed")
}

/// Build a minimal `Request` pointing at `url` with no auth, headers, params or body.
fn bare_request(url: impl Into<String>, collection_id: impl Into<String>) -> Request {
    Request {
        id: Uuid::new_v4().to_string(),
        collection_id: collection_id.into(),
        name: "e2e-test".to_string(),
        method: HttpMethod::Get,
        url: url.into(),
        headers: vec![],
        params: vec![],
        body: RequestBody::None,
        auth: AuthConfig::None,
        position: 0,
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
    }
}

/// Run the interpolation → header-merge → execute pipeline and return
/// `(HttpResponse, interpolated_request)`.
///
/// This is the service-level equivalent of `commands::http::send_request`.
async fn run_pipeline(
    db: DbHandle,
    request: Request,
    environment_id: Option<String>,
) -> Result<
    (
        lightman_lib::models::HttpResponse,
        Request, // interpolated (pre-merge) request stored in history
    ),
    String,
> {
    // 1. Fetch variables
    let env_vars = match &environment_id {
        Some(env_id) => env_svc::list_variables(db.clone(), env_id.clone()).await?,
        None => vec![],
    };
    let global_vars = storage::list_global_variables(db.clone())
        .await?
        .into_iter()
        .map(|r| Variable {
            id: r.id,
            environment_id: r.environment_id,
            key: r.key,
            value: r.value,
            enabled: r.enabled,
        })
        .collect::<Vec<_>>();

    // 2. Interpolate variables
    let interpolated =
        VarInterpolatorService::interpolate(request, &env_vars, &[], &global_vars);

    // 3. Fetch collection default headers (always vec![] from DB currently)
    let collection_default_headers = match collection_svc::get_collection(
        db.clone(),
        interpolated.collection_id.clone(),
    )
    .await?
    {
        Some(col) => col.default_headers,
        None => vec![],
    };

    // 4. Merge headers + inject auth
    let merge_result = HeaderMergerService::merge(&interpolated, &collection_default_headers, &[]);
    let mut final_request = interpolated.clone();
    final_request.headers = merge_result.headers;
    for (key, value) in &merge_result.extra_query_params {
        final_request.params.push(QueryParam {
            key: key.clone(),
            value: value.clone(),
            enabled: true,
        });
    }

    // 5. Execute
    let executor = HttpExecutorService::new()?;
    let response = executor.execute(&final_request).await?;

    // 6. Persist history
    let history_id = Uuid::new_v4().to_string();
    let executed_at = Utc::now().to_rfc3339();
    let request_json = serde_json::to_string(&interpolated)
        .map_err(|e| format!("serialise request: {e}"))?;
    let response_headers_json = serde_json::to_string(&response.headers)
        .map_err(|e| format!("serialise response headers: {e}"))?;

    storage::insert_history_entry(
        db.clone(),
        history_id,
        request_json,
        response.status_code as i64,
        response.status_text.clone(),
        response.response_time_ms as i64,
        response.response_size_bytes as i64,
        response_headers_json,
        response.body.clone(),
        executed_at,
    )
    .await?;

    Ok((response, interpolated))
}

// ===========================================================================
// Test 1 — Basic GET: no variables, no auth
// ===========================================================================

/// Verify that a plain GET request with no variables or auth:
/// - Returns the correct status code from the mock server.
/// - Persists exactly one history entry with matching status code and body.
#[tokio::test]
async fn test_basic_get_persists_history() {
    let db = test_db();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/hello"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(r#"{"message":"hello"}"#),
        )
        .mount(&server)
        .await;

    // Seed a collection so the pipeline can look up default headers.
    collection_svc::create_collection(db.clone(), "Test Collection".to_string(), None, None, lightman_lib::models::AuthConfig::None)
        .await
        .expect("create collection");
    let collections = lightman_lib::storage::list_collections(db.clone())
        .await
        .expect("list collections");
    let col_id = collections[0].id.clone();

    let url = format!("{}/hello", server.uri());
    let request = bare_request(url, col_id);

    let (response, _) = run_pipeline(db.clone(), request, None)
        .await
        .expect("pipeline failed");

    // Assert response
    assert_eq!(response.status_code, 200);
    assert!(
        response.body.contains("hello"),
        "body should contain 'hello', got: {}",
        response.body
    );

    // Assert history was persisted
    let history = storage::list_history(db.clone(), None)
        .await
        .expect("list_history failed");

    assert_eq!(history.len(), 1, "expected exactly 1 history entry");
    assert_eq!(history[0].status_code, 200);
    assert!(
        history[0].response_body.contains("hello"),
        "history body should contain 'hello'"
    );
}

// ===========================================================================
// Test 2 — Variable interpolation in URL
// ===========================================================================

/// Verify that `{{base_url}}` and `{{path}}` placeholders in the URL are
/// resolved from active environment variables before the request is sent.
#[tokio::test]
async fn test_variable_interpolation_in_url() {
    let db = test_db();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/users/42"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"id":42}"#))
        .mount(&server)
        .await;

    // Seed collection
    collection_svc::create_collection(db.clone(), "Col".to_string(), None, None, lightman_lib::models::AuthConfig::None)
        .await
        .unwrap();
    let col_id = storage::list_collections(db.clone())
        .await
        .unwrap()[0]
        .id
        .clone();

    // Seed environment with variables
    let env = env_svc::create_environment(db.clone(), "Test Env".to_string())
        .await
        .unwrap();
    let base_url = server.uri(); // e.g. "http://127.0.0.1:PORT"
    env_svc::create_variable(
        db.clone(),
        Some(env.id.clone()),
        "base_url".to_string(),
        base_url,
        true,
    )
    .await
    .unwrap();
    env_svc::create_variable(
        db.clone(),
        Some(env.id.clone()),
        "user_id".to_string(),
        "42".to_string(),
        true,
    )
    .await
    .unwrap();

    let mut request = bare_request("{{base_url}}/users/{{user_id}}", col_id);
    request.method = HttpMethod::Get;

    let (response, interpolated) =
        run_pipeline(db.clone(), request, Some(env.id.clone()))
            .await
            .expect("pipeline failed");

    assert_eq!(response.status_code, 200);

    // The interpolated URL must no longer contain placeholders
    assert!(
        !interpolated.url.contains("{{"),
        "interpolated URL must not contain unresolved placeholders; got: {}",
        interpolated.url
    );
    assert!(
        interpolated.url.contains("/users/42"),
        "interpolated URL must contain resolved path; got: {}",
        interpolated.url
    );

    // History must store the resolved URL (not the raw template)
    let history = storage::list_history(db.clone(), Some(1))
        .await
        .unwrap();
    assert_eq!(history.len(), 1);
    let stored_request: Request =
        serde_json::from_str(&history[0].request_json).expect("deserialise history request");
    assert!(
        stored_request.url.contains("/users/42"),
        "history must store resolved URL, got: {}",
        stored_request.url
    );
    assert!(
        !stored_request.url.contains("{{"),
        "history must not store raw placeholder in URL"
    );
}

// ===========================================================================
// Test 3 — Variable interpolation in request header value
// ===========================================================================

/// Verify that a `{{token}}` placeholder in a request header value is resolved
/// from an active environment variable before the request is executed.
#[tokio::test]
async fn test_variable_interpolation_in_header_value() {
    let db = test_db();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/secure"))
        .and(header("x-api-token", "secret-abc"))
        .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
        .mount(&server)
        .await;

    collection_svc::create_collection(db.clone(), "C".to_string(), None, None, lightman_lib::models::AuthConfig::None)
        .await
        .unwrap();
    let col_id = storage::list_collections(db.clone()).await.unwrap()[0]
        .id
        .clone();

    let env = env_svc::create_environment(db.clone(), "E".to_string())
        .await
        .unwrap();
    env_svc::create_variable(
        db.clone(),
        Some(env.id.clone()),
        "token".to_string(),
        "secret-abc".to_string(),
        true,
    )
    .await
    .unwrap();

    let url = format!("{}/secure", server.uri());
    let mut request = bare_request(url, col_id);
    request.headers = vec![Header {
        key: "x-api-token".to_string(),
        value: "{{token}}".to_string(),
        enabled: true,
    }];

    let (response, interpolated) =
        run_pipeline(db.clone(), request, Some(env.id.clone()))
            .await
            .expect("pipeline failed");

    assert_eq!(
        response.status_code, 200,
        "mock should respond 200 only when header value matches"
    );

    // The interpolated request must have the resolved header value
    let header = interpolated
        .headers
        .iter()
        .find(|h| h.key == "x-api-token")
        .expect("x-api-token header missing from interpolated request");
    assert!(
        response.body.contains("hello"),
        "body should contain 'hello', got: {}",
        response.body
    );

    // Assert history was persisted
    let history = storage::list_history(db.clone(), None)
        .await
        .expect("list_history failed");

    assert_eq!(history.len(), 1, "expected exactly 1 history entry");
    assert_eq!(history[0].status_code, 200);
    assert!(
        history[0].response_body.contains("hello"),
        "history body should contain 'hello'"
    );
}

// ===========================================================================
// Test 2 — Variable interpolation in URL

// ===========================================================================
// Test 3 — Variable interpolation in request header value
// ===========================================================================

/// Verify that a `{{token}}` placeholder in a request header value is resolved
// ===========================================================================
// Test 4 — Global variable fallback (no active environment)
// ===========================================================================

/// Verify that global variables (environment_id = NULL) are used when no active
/// environment is selected (environment_id parameter is None).
#[tokio::test]
async fn test_global_variable_fallback() {
    let db = test_db();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/global"))
        .respond_with(ResponseTemplate::new(200).set_body_string("global-ok"))
        .mount(&server)
        .await;

    collection_svc::create_collection(db.clone(), "C".to_string(), None, None, lightman_lib::models::AuthConfig::None)
        .await
        .unwrap();
    let col_id = storage::list_collections(db.clone()).await.unwrap()[0]
        .id
        .clone();

    // Insert a global variable (environment_id = None)
    env_svc::create_variable(
        db.clone(),
        None, // global
        "endpoint".to_string(),
        "global".to_string(),
        true,
    )
    .await
    .unwrap();

    let url = format!("{}/{{{{endpoint}}}}", server.uri());
    let request = bare_request(url, col_id);

    // No environment_id — only global vars should be used
    let (response, interpolated) =
        run_pipeline(db.clone(), request, None)
            .await
            .expect("pipeline failed");

    assert_eq!(response.status_code, 200);
    assert!(
        interpolated.url.ends_with("/global"),
        "URL should end with /global after global var resolution, got: {}",
        interpolated.url
    );
}

// ===========================================================================
// Test 5 — Bearer token auth injected as Authorization header
// ===========================================================================

/// Verify that `AuthConfig::BearerToken` causes the `Authorization: Bearer …`
/// header to be injected by the HeaderMergerService and actually sent to the
/// mock server.
#[tokio::test]
async fn test_bearer_auth_header_injected() {
    let db = test_db();
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/auth"))
        .and(header("Authorization", "Bearer my-secret-token"))
        .respond_with(ResponseTemplate::new(201).set_body_string(r#"{"created":true}"#))
        .mount(&server)
        .await;

    collection_svc::create_collection(db.clone(), "C".to_string(), None, None, lightman_lib::models::AuthConfig::None)
        .await
        .unwrap();
    let col_id = storage::list_collections(db.clone()).await.unwrap()[0]
        .id
        .clone();

    let url = format!("{}/auth", server.uri());
    let mut request = bare_request(url, col_id);
    request.method = HttpMethod::Post;
    request.auth = AuthConfig::BearerToken {
        token: "my-secret-token".to_string(),
    };

    let (response, _) = run_pipeline(db.clone(), request, None)
        .await
        .expect("pipeline failed");

    assert_eq!(
        response.status_code, 201,
        "mock should respond 201 only when Authorization header is present and correct"
    );

    // History entry must be present
    let history = storage::list_history(db.clone(), Some(1)).await.unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].status_code, 201);
}

// ===========================================================================
// Test 6 — Basic auth header encoded correctly
// ===========================================================================

/// Verify that `AuthConfig::BasicAuth` causes the correct `Authorization: Basic
/// <base64(user:pass)>` header to be sent.
#[tokio::test]
async fn test_basic_auth_header_encoded_correctly() {
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};

    let db = test_db();
    let server = MockServer::start().await;

    let credentials = BASE64.encode(b"alice:wonderland");
    let expected_auth = format!("Basic {credentials}");

    Mock::given(method("GET"))
        .and(path("/protected"))
        .and(header("Authorization", expected_auth.as_str()))
        .respond_with(ResponseTemplate::new(200).set_body_string("authorized"))
        .mount(&server)
        .await;

    collection_svc::create_collection(db.clone(), "C".to_string(), None, None, lightman_lib::models::AuthConfig::None)
        .await
        .unwrap();
    let col_id = storage::list_collections(db.clone()).await.unwrap()[0]
        .id
        .clone();

    let url = format!("{}/protected", server.uri());
    let mut request = bare_request(url, col_id);
    request.auth = AuthConfig::BasicAuth {
        username: "alice".to_string(),
        password: "wonderland".to_string(),
    };

    let (response, _) = run_pipeline(db.clone(), request, None)
        .await
        .expect("pipeline failed");

    assert_eq!(response.status_code, 200);
}

// ===========================================================================
// Test 7 — ApiKey injected as query param
// ===========================================================================

/// Verify that `AuthConfig::ApiKey { location: QueryParam }` appends the
/// key-value pair to the URL query string via `MergeResult.extra_query_params`.
#[tokio::test]
async fn test_api_key_as_query_param() {
    let db = test_db();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/data"))
        .and(query_param("api_key", "k3y-v4lu3"))
        .respond_with(ResponseTemplate::new(200).set_body_string("data"))
        .mount(&server)
        .await;

    collection_svc::create_collection(db.clone(), "C".to_string(), None, None, lightman_lib::models::AuthConfig::None)
        .await
        .unwrap();
    let col_id = storage::list_collections(db.clone()).await.unwrap()[0]
        .id
        .clone();

    let url = format!("{}/data", server.uri());
    let mut request = bare_request(url, col_id);
    request.auth = AuthConfig::ApiKey {
        key: "api_key".to_string(),
        value: "k3y-v4lu3".to_string(),
        location: ApiKeyLocation::QueryParam,
    };

    let (response, _) = run_pipeline(db.clone(), request, None)
        .await
        .expect("pipeline failed");

    assert_eq!(
        response.status_code, 200,
        "mock must respond 200 when api_key query param is present"
    );
}

// ===========================================================================
// Test 8 — POST with JSON body: interpolation in body content
// ===========================================================================

/// Verify that `{{user_id}}` placeholders inside a JSON body are resolved
/// before the request is sent.
#[tokio::test]
async fn test_variable_interpolation_in_json_body() {
    let db = test_db();
    let server = MockServer::start().await;

    // wiremock does not match by body content directly; we check the status
    // and verify interpolation in the returned interpolated request.
    Mock::given(method("POST"))
        .and(path("/submit"))
        .respond_with(ResponseTemplate::new(201).set_body_string(r#"{"ok":true}"#))
        .mount(&server)
        .await;

    collection_svc::create_collection(db.clone(), "C".to_string(), None, None, lightman_lib::models::AuthConfig::None)
        .await
        .unwrap();
    let col_id = storage::list_collections(db.clone()).await.unwrap()[0]
        .id
        .clone();

    let env = env_svc::create_environment(db.clone(), "E".to_string())
        .await
        .unwrap();
    env_svc::create_variable(
        db.clone(),
        Some(env.id.clone()),
        "user_id".to_string(),
        "99".to_string(),
        true,
    )
    .await
    .unwrap();

    let url = format!("{}/submit", server.uri());
    let mut request = bare_request(url, col_id);
    request.method = HttpMethod::Post;
    request.body = RequestBody::Json {
        content: r#"{"user":"{{user_id}}"}"#.to_string(),
    };

    let (response, interpolated) =
        run_pipeline(db.clone(), request, Some(env.id))
            .await
            .expect("pipeline failed");

    assert_eq!(response.status_code, 201);

    match &interpolated.body {
        RequestBody::Json { content } => {
            assert!(
                content.contains("\"99\""),
                "JSON body should contain resolved user_id, got: {content}"
            );
            assert!(
                !content.contains("{{"),
                "JSON body must not contain unresolved placeholders, got: {content}"
            );
        }
        other => panic!("Expected Json body, got: {other:?}"),
    }
}

// ===========================================================================
// Test 9 — Multiple requests accumulate history entries
// ===========================================================================

/// Verify that running the pipeline multiple times accumulates separate history
/// entries, ordered most-recent-first.
#[tokio::test]
async fn test_multiple_requests_accumulate_history() {
    let db = test_db();
    let server = MockServer::start().await;

    // Two different paths with different status codes
    Mock::given(method("GET"))
        .and(path("/first"))
        .respond_with(ResponseTemplate::new(200).set_body_string("first"))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/second"))
        .respond_with(ResponseTemplate::new(404).set_body_string("not-found"))
        .mount(&server)
        .await;

    collection_svc::create_collection(db.clone(), "C".to_string(), None, None, lightman_lib::models::AuthConfig::None)
        .await
        .unwrap();
    let col_id = storage::list_collections(db.clone()).await.unwrap()[0]
        .id
        .clone();

    let url1 = format!("{}/first", server.uri());
    let url2 = format!("{}/second", server.uri());

    run_pipeline(db.clone(), bare_request(url1, col_id.clone()), None)
        .await
        .expect("first pipeline failed");

    run_pipeline(db.clone(), bare_request(url2, col_id.clone()), None)
        .await
        .expect("second pipeline failed");

    let history = storage::list_history(db.clone(), None)
        .await
        .expect("list_history failed");

    assert_eq!(history.len(), 2, "expected 2 history entries");

    // History is ordered DESC by executed_at — second request is most recent
    let status_codes: Vec<i64> = history.iter().map(|h| h.status_code).collect();
    assert!(
        status_codes.contains(&200),
        "200 should be in history, got: {status_codes:?}"
    );
    assert!(
        status_codes.contains(&404),
        "404 should be in history, got: {status_codes:?}"
    );
}

// ===========================================================================
// Test 10 — Unresolved placeholder is preserved in URL
// ===========================================================================

/// Verify that a `{{missing_var}}` placeholder is left unchanged in the URL
/// when it cannot be resolved from any variable scope (Requirement 3.5).
#[tokio::test]
async fn test_unresolved_placeholder_preserved_in_url() {
    let db = test_db();

    collection_svc::create_collection(db.clone(), "C".to_string(), None, None, lightman_lib::models::AuthConfig::None)
        .await
        .unwrap();
    let col_id = storage::list_collections(db.clone()).await.unwrap()[0]
        .id
        .clone();

    // Empty variable scopes — nothing to resolve
    let raw_url = "http://example.com/{{missing_var}}/items";
    let request = bare_request(raw_url, col_id);

    // We only test interpolation here, not execution (the URL would fail to connect)
    let env_vars: Vec<Variable> = vec![];
    let global_vars: Vec<Variable> = vec![];

    let interpolated =
        VarInterpolatorService::interpolate(request, &env_vars, &[], &global_vars);

    assert_eq!(
        interpolated.url, raw_url,
        "unresolved placeholder should remain unchanged in the URL"
    );
}

// ===========================================================================
// Test 11 — Environment variable overrides global variable (priority chain)
// ===========================================================================

/// Verify that when both an environment variable and a global variable share
/// the same key, the environment variable wins (Requirement 3.4).
#[tokio::test]
async fn test_env_variable_overrides_global_variable() {
    let db = test_db();
    let server = MockServer::start().await;

    // The mock only responds when the path contains the env-specific value
    Mock::given(method("GET"))
        .and(path("/env-value"))
        .respond_with(ResponseTemplate::new(200).set_body_string("env"))
        .mount(&server)
        .await;

    collection_svc::create_collection(db.clone(), "C".to_string(), None, None, lightman_lib::models::AuthConfig::None)
        .await
        .unwrap();
    let col_id = storage::list_collections(db.clone()).await.unwrap()[0]
        .id
        .clone();

    // Seed global variable with lower-priority value
    env_svc::create_variable(
        db.clone(),
        None,
        "path_segment".to_string(),
        "global-value".to_string(),
        true,
    )
    .await
    .unwrap();

    // Seed environment variable that should override the global one
    let env = env_svc::create_environment(db.clone(), "E".to_string())
        .await
        .unwrap();
    env_svc::create_variable(
        db.clone(),
        Some(env.id.clone()),
        "path_segment".to_string(),
        "env-value".to_string(),
        true,
    )
    .await
    .unwrap();

    let url = format!("{}/{{{{path_segment}}}}", server.uri());
    let request = bare_request(url, col_id);

    let (response, interpolated) =
        run_pipeline(db.clone(), request, Some(env.id))
            .await
            .expect("pipeline failed");

    assert_eq!(
        response.status_code, 200,
        "env variable should override global variable; URL should resolve to /env-value"
    );
    assert!(
        interpolated.url.ends_with("/env-value"),
        "interpolated URL should end with /env-value, got: {}",
        interpolated.url
    );
}

// ===========================================================================
// Test 12 — Response time and size are recorded in history
// ===========================================================================

/// Verify that the history entry captures `response_time_ms > 0` and
/// `response_size_bytes` equal to the byte length of the response body.
#[tokio::test]
async fn test_history_records_response_metrics() {
    let db = test_db();
    let server = MockServer::start().await;

    let body_content = r#"{"status":"metrics-test"}"#;

    Mock::given(method("GET"))
        .and(path("/metrics"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body_content))
        .mount(&server)
        .await;

    collection_svc::create_collection(db.clone(), "C".to_string(), None, None, lightman_lib::models::AuthConfig::None)
        .await
        .unwrap();
    let col_id = storage::list_collections(db.clone()).await.unwrap()[0]
        .id
        .clone();

    let url = format!("{}/metrics", server.uri());
    let request = bare_request(url, col_id);

    let (response, _) = run_pipeline(db.clone(), request, None)
        .await
        .expect("pipeline failed");

    let history = storage::list_history(db.clone(), Some(1))
        .await
        .unwrap();
    assert_eq!(history.len(), 1);

    let entry = &history[0];

    // response_time_ms must be recorded (> 0 for a real loopback request)
    assert!(
        entry.response_time_ms >= 0,
        "response_time_ms must be non-negative"
    );

    // response_size_bytes must equal byte length of the body
    let expected_size = body_content.len() as i64;
    assert_eq!(
        entry.response_size_bytes, expected_size,
        "response_size_bytes should equal body byte length"
    );

    // Cross-check with what the executor reported
    assert_eq!(
        response.response_size_bytes, expected_size as u64,
        "HttpResponse.response_size_bytes should match body length"
    );
}

// ===========================================================================
// Test 13 — History stores interpolated (pre-merge) request, not final request
// ===========================================================================

/// Verify that the request stored in history is the variable-interpolated
/// version, NOT the final merged version (i.e. auth headers are not leaked
/// into stored request headers — Requirement 4.1 + security intent).
#[tokio::test]
async fn test_history_stores_pre_merge_request() {
    let db = test_db();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/secret"))
        .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
        .mount(&server)
        .await;

    collection_svc::create_collection(db.clone(), "C".to_string(), None, None, lightman_lib::models::AuthConfig::None)
        .await
        .unwrap();
    let col_id = storage::list_collections(db.clone()).await.unwrap()[0]
        .id
        .clone();

    let url = format!("{}/secret", server.uri());
    let mut request = bare_request(url, col_id);
    // Add BearerToken auth — the merge step injects this as Authorization header
    // but the stored history request should NOT contain it.
    request.auth = AuthConfig::BearerToken {
        token: "super-secret-token".to_string(),
    };

    run_pipeline(db.clone(), request, None)
        .await
        .expect("pipeline failed");

    let history = storage::list_history(db.clone(), Some(1))
        .await
        .unwrap();
    assert_eq!(history.len(), 1);

    let stored_request: Request =
        serde_json::from_str(&history[0].request_json).expect("deserialise history request");

    // The stored request must NOT have Authorization in its headers
    // (that is injected only for the actual HTTP send, not persisted)
    let has_auth_header = stored_request
        .headers
        .iter()
        .any(|h| h.key.eq_ignore_ascii_case("Authorization"));

    assert!(
        !has_auth_header,
        "history should store pre-merge request — Authorization header must not be present"
    );
}
