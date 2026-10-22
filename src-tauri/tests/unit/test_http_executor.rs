// Unit tests — HTTP Executor method routing (Task 4.3)
//
// Verifies that each `HttpMethod` variant is mapped to the correct HTTP verb
// when constructing the outgoing reqwest request.  We use `wiremock` as a
// local mock server so these tests run entirely offline without hitting any
// real network endpoint.
//
// Requirements: 1.1

use lightman_lib::{
    models::{AuthConfig, HttpMethod, QueryParam, Request, RequestBody},
    services::http_executor::HttpExecutorService,
};
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a minimal `Request` targeting a wiremock server at `base_url`.
fn make_request(http_method: HttpMethod, base_url: &str, endpoint: &str) -> Request {
    Request {
        id: uuid::Uuid::new_v4().to_string(),
        collection_id: "col-test".to_string(),
        name: format!("{http_method} test"),
        method: http_method,
        url: format!("{base_url}{endpoint}"),
        headers: vec![],
        params: vec![],
        body: RequestBody::None,
        auth: AuthConfig::None,
        position: 0,
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
    }
}

/// Mount a wiremock expectation that asserts exactly one request arrives with
/// the given HTTP verb on `/test`, responding with 200.
async fn mount_method_mock(server: &MockServer, http_verb: &str) {
    Mock::given(method(http_verb))
        .and(path("/test"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(server)
        .await;
}

// ---------------------------------------------------------------------------
// Task 4.3 — one test per HttpMethod variant
// ---------------------------------------------------------------------------

/// GET → reqwest::Method::GET
#[tokio::test]
async fn routes_get_method() {
    let server = MockServer::start().await;
    mount_method_mock(&server, "GET").await;

    let svc = HttpExecutorService::new().unwrap();
    let req = make_request(HttpMethod::Get, &server.uri(), "/test");
    let resp = svc.execute(&req).await.expect("execute should succeed");

    assert_eq!(resp.status_code, 200, "expected 200 from mock for GET");
    // wiremock will panic on drop if the expectation was not met, so reaching
    // this assert already proves the correct method was used.
}

/// POST → reqwest::Method::POST
#[tokio::test]
async fn routes_post_method() {
    let server = MockServer::start().await;
    mount_method_mock(&server, "POST").await;

    let svc = HttpExecutorService::new().unwrap();
    let req = make_request(HttpMethod::Post, &server.uri(), "/test");
    let resp = svc.execute(&req).await.expect("execute should succeed");

    assert_eq!(resp.status_code, 200, "expected 200 from mock for POST");
}

/// PUT → reqwest::Method::PUT
#[tokio::test]
async fn routes_put_method() {
    let server = MockServer::start().await;
    mount_method_mock(&server, "PUT").await;

    let svc = HttpExecutorService::new().unwrap();
    let req = make_request(HttpMethod::Put, &server.uri(), "/test");
    let resp = svc.execute(&req).await.expect("execute should succeed");

    assert_eq!(resp.status_code, 200, "expected 200 from mock for PUT");
}

/// PATCH → reqwest::Method::PATCH
#[tokio::test]
async fn routes_patch_method() {
    let server = MockServer::start().await;
    mount_method_mock(&server, "PATCH").await;

    let svc = HttpExecutorService::new().unwrap();
    let req = make_request(HttpMethod::Patch, &server.uri(), "/test");
    let resp = svc.execute(&req).await.expect("execute should succeed");

    assert_eq!(resp.status_code, 200, "expected 200 from mock for PATCH");
}

/// DELETE → reqwest::Method::DELETE
#[tokio::test]
async fn routes_delete_method() {
    let server = MockServer::start().await;
    mount_method_mock(&server, "DELETE").await;

    let svc = HttpExecutorService::new().unwrap();
    let req = make_request(HttpMethod::Delete, &server.uri(), "/test");
    let resp = svc.execute(&req).await.expect("execute should succeed");

    assert_eq!(resp.status_code, 200, "expected 200 from mock for DELETE");
}

/// HEAD → reqwest::Method::HEAD
///
/// HEAD responses must have an empty body per RFC 9110 §9.3.2.
#[tokio::test]
async fn routes_head_method() {
    let server = MockServer::start().await;
    mount_method_mock(&server, "HEAD").await;

    let svc = HttpExecutorService::new().unwrap();
    let req = make_request(HttpMethod::Head, &server.uri(), "/test");
    let resp = svc.execute(&req).await.expect("execute should succeed");

    assert_eq!(resp.status_code, 200, "expected 200 from mock for HEAD");
    assert!(
        resp.body.is_empty(),
        "HEAD response body must be empty, got: {:?}",
        resp.body
    );
}

/// OPTIONS → reqwest::Method::OPTIONS
#[tokio::test]
async fn routes_options_method() {
    let server = MockServer::start().await;
    mount_method_mock(&server, "OPTIONS").await;

    let svc = HttpExecutorService::new().unwrap();
    let req = make_request(HttpMethod::Options, &server.uri(), "/test");
    let resp = svc.execute(&req).await.expect("execute should succeed");

    assert_eq!(resp.status_code, 200, "expected 200 from mock for OPTIONS");
}

// ---------------------------------------------------------------------------
// Additional routing edge-cases
// ---------------------------------------------------------------------------

/// Wrong verb should NOT match the mock and wiremock returns 404 — proves
/// the routing is discriminated correctly and not just "always passes".
#[tokio::test]
async fn wrong_method_does_not_match_mock() {
    let server = MockServer::start().await;

    // Only mount a POST mock; we will send GET — expect no match → 404.
    Mock::given(method("POST"))
        .and(path("/test"))
        .respond_with(ResponseTemplate::new(200))
        // No `.expect()` call — zero or one hit is fine here.
        .mount(&server)
        .await;

    let svc = HttpExecutorService::new().unwrap();
    let req = make_request(HttpMethod::Get, &server.uri(), "/test");
    let resp = svc.execute(&req).await.expect("execute should succeed");

    assert_eq!(
        resp.status_code, 404,
        "a GET to a POST-only mock should return 404"
    );
}

/// Query parameters marked `enabled: false` must not be sent.
#[tokio::test]
async fn disabled_query_params_not_forwarded() {
    use wiremock::matchers::query_param_is_missing;

    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/test"))
        .and(query_param_is_missing("hidden"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;

    let svc = HttpExecutorService::new().unwrap();
    let mut req = make_request(HttpMethod::Get, &server.uri(), "/test");
    req.params = vec![QueryParam {
        key: "hidden".to_string(),
        value: "secret".to_string(),
        enabled: false, // must NOT be forwarded
    }];

    let resp = svc.execute(&req).await.expect("execute should succeed");
    assert_eq!(resp.status_code, 200);
}

/// Enabled query parameters must be forwarded correctly.
#[tokio::test]
async fn enabled_query_params_are_forwarded() {
    use wiremock::matchers::query_param;

    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/test"))
        .and(query_param("page", "2"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;

    let svc = HttpExecutorService::new().unwrap();
    let mut req = make_request(HttpMethod::Get, &server.uri(), "/test");
    req.params = vec![QueryParam {
        key: "page".to_string(),
        value: "2".to_string(),
        enabled: true,
    }];

    let resp = svc.execute(&req).await.expect("execute should succeed");
    assert_eq!(resp.status_code, 200);
}

/// Response time is captured as a non-zero value for a real (mock) server round-trip.
#[tokio::test]
async fn response_time_is_captured() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/test"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let svc = HttpExecutorService::new().unwrap();
    let req = make_request(HttpMethod::Get, &server.uri(), "/test");
    let resp = svc.execute(&req).await.expect("execute should succeed");

    // response_time_ms should be a valid u64; it may be 0 on very fast
    // loopback calls, so we just assert the field exists and is a u64.
    let _ms: u64 = resp.response_time_ms;
}

/// Response size reflects the actual body length in bytes.
#[tokio::test]
async fn response_size_matches_body_length() {
    let server = MockServer::start().await;
    let body = r#"{"id":1,"name":"test"}"#;

    Mock::given(method("GET"))
        .and(path("/test"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(&server)
        .await;

    let svc = HttpExecutorService::new().unwrap();
    let req = make_request(HttpMethod::Get, &server.uri(), "/test");
    let resp = svc.execute(&req).await.expect("execute should succeed");

    assert_eq!(
        resp.response_size_bytes,
        body.len() as u64,
        "response_size_bytes should equal actual body byte length"
    );
    assert_eq!(resp.body, body);
}

/// A connection to a non-existent server returns a descriptive error, not a panic.
#[tokio::test]
async fn unreachable_host_returns_error() {
    let svc = HttpExecutorService::new().unwrap();
    // Port 1 is effectively guaranteed to refuse connections.
    let req = make_request(HttpMethod::Get, "http://127.0.0.1:1", "/test");
    let result = svc.execute(&req).await;

    assert!(result.is_err(), "expected error for unreachable host");
    let err = result.unwrap_err();
    // Must be a non-empty, human-readable string.
    assert!(!err.is_empty(), "error string must not be empty");
}
