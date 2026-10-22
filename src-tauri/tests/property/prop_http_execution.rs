// Property-based tests for HTTP execution (Properties 4, 8, 9).
//
// Task 4.4:
//   - Property 4:  HTTP Method Round-Trip
//   - Property 8:  HTTP Executor Returns Status Code for Any Valid Request
//   - Property 9:  Response Payload Integrity
//
// Validates: Requirements 1.1, 1.2, 1.5
//
// All tests use `wiremock` as a local mock server so they run fully offline and
// deterministically.  proptest drives the inputs.

use lightman_lib::{
    models::{AuthConfig, Header, HttpMethod, QueryParam, Request, RequestBody},
    services::http_executor::HttpExecutorService,
};
use proptest::prelude::*;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------------------------------------------------------------------------
// Arbitrary generators
// ---------------------------------------------------------------------------

/// All seven HTTP methods supported by Lightman.
fn arb_http_method() -> impl Strategy<Value = HttpMethod> {
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

/// The reqwest method name string that matches the Lightman `HttpMethod` variant.
fn method_str(m: &HttpMethod) -> &'static str {
    match m {
        HttpMethod::Get => "GET",
        HttpMethod::Post => "POST",
        HttpMethod::Put => "PUT",
        HttpMethod::Patch => "PATCH",
        HttpMethod::Delete => "DELETE",
        HttpMethod::Head => "HEAD",
        HttpMethod::Options => "OPTIONS",
    }
}

/// Generate a simple ASCII header key safe for HTTP (no special chars).
fn arb_header_key() -> impl Strategy<Value = String> {
    "[a-zA-Z][a-zA-Z0-9\\-]{1,20}".prop_map(|s| s)
}

/// Generate a printable ASCII header value.
fn arb_header_value() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9 /._\\-]{1,40}".prop_map(|s| s)
}

/// Generate a list of 0–4 enabled headers with safe ASCII keys/values.
fn arb_headers() -> impl Strategy<Value = Vec<Header>> {
    prop::collection::vec(
        (arb_header_key(), arb_header_value()).prop_map(|(k, v)| Header {
            key: k,
            value: v,
            enabled: true,
        }),
        0..=4,
    )
}

/// Generate a list of 0–4 enabled query parameters.
fn arb_query_params() -> impl Strategy<Value = Vec<QueryParam>> {
    prop::collection::vec(
        ("[a-z]{1,10}", "[a-zA-Z0-9]{1,20}").prop_map(|(k, v)| QueryParam {
            key: k,
            value: v,
            enabled: true,
        }),
        0..=4,
    )
}

/// Generate body text that is valid UTF-8, length 0–512 bytes.
fn arb_body_text() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9 .,!?]{0,512}".prop_map(|s| s)
}

/// Build a minimal `Request` for the given method pointing at a mock server URL.
fn make_request(method: HttpMethod, url: String) -> Request {
    Request {
        id: uuid::Uuid::new_v4().to_string(),
        collection_id: "col-test".to_string(),
        name: "prop-test".to_string(),
        method,
        url,
        headers: vec![],
        params: vec![],
        body: RequestBody::None,
        auth: AuthConfig::None,
        position: 0,
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Property 4 — HTTP Method Round-Trip
//
// For every HttpMethod variant the executor must send exactly that HTTP verb.
// We verify this by registering a method-specific mock and asserting the mock
// was called exactly once (wiremock panics if the expectation is not met).
// ---------------------------------------------------------------------------

/// Run a single method-round-trip check against a fresh mock server.
///
/// This async helper is called from a proptest runner via `tokio::runtime`.
async fn prop4_check_method(http_method: HttpMethod) {
    let server = MockServer::start().await;

    // Mount a mock that responds 200 only for the correct HTTP verb.
    Mock::given(method(method_str(&http_method)))
        .and(path("/probe"))
        .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
        .expect(1) // must be called exactly once
        .mount(&server)
        .await;

    let url = format!("{}/probe", server.uri());
    let request = make_request(http_method, url);

    let svc = HttpExecutorService::new().expect("executor init");
    let resp = svc
        .execute(&request)
        .await
        .expect("execute should succeed");

    // HEAD responses legitimately have an empty body — all others should not.
    if request.method != HttpMethod::Head {
        assert_eq!(
            resp.status_code, 200,
            "expected 200 from mock, got {}",
            resp.status_code
        );
    }

    // wiremock verifies the expectation (exactly 1 call) on server drop.
}

proptest! {
    // ---------------------------------------------------------------------------
    // Property 4 — HTTP Method Round-Trip (100 cases)
    // ---------------------------------------------------------------------------
    #[test]
    fn prop4_http_method_round_trip(method in arb_http_method()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(prop4_check_method(method));
    }
}

// ---------------------------------------------------------------------------
// Property 8 — HTTP Executor Returns Status Code for Any Valid Request
//
// For any combination of HTTP method, headers, and query params the executor
// must:
//   - NOT return an Err (network-level failures are not expected against a
//     local mock server).
//   - Return a valid HTTP status code (100–599).
//   - Populate `response_time_ms` (>= 0, trivially true for u64).
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn prop8_executor_always_returns_status_code(
        http_method in arb_http_method(),
        extra_headers in arb_headers(),
        query_params in arb_query_params(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let (is_ok, err_msg, status_code) = rt.block_on(async {
            let server = MockServer::start().await;

            // Respond 200 to any request on /anything.
            Mock::given(path("/anything"))
                .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
                .mount(&server)
                .await;

            let url = format!("{}/anything", server.uri());
            let mut request = make_request(http_method, url);
            request.headers = extra_headers;
            request.params = query_params;

            let svc = HttpExecutorService::new().expect("executor init");
            match svc.execute(&request).await {
                Ok(resp) => (true, String::new(), resp.status_code),
                Err(e) => (false, e, 0),
            }
        });

        prop_assert!(is_ok, "execute returned Err: {}", err_msg);
        prop_assert!(
            (100..=599).contains(&status_code),
            "status code out of range: {}",
            status_code
        );
    }
}

// ---------------------------------------------------------------------------
// Property 9 — Response Payload Integrity
//
// The executor must:
//   a) Return the exact body bytes sent by the server (UTF-8 case).
//   b) Set `response_size_bytes` equal to the byte length of the raw body.
//   c) Include response headers emitted by the server.
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn prop9_response_payload_integrity(
        body_text in arb_body_text(),
    ) {
        // Skip trivial empty case to keep the assertion meaningful.
        prop_assume!(!body_text.is_empty());

        let body_bytes = body_text.as_bytes().to_vec();
        let expected_size = body_bytes.len() as u64;
        let body_text_clone = body_text.clone();

        let rt = tokio::runtime::Runtime::new().unwrap();
        let (result_body, result_size, has_custom) = rt.block_on(async {
            let server = MockServer::start().await;

            Mock::given(method("GET"))
                .and(path("/payload"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_bytes(body_bytes.clone())
                        .insert_header("x-custom", "integrity-check"),
                )
                .mount(&server)
                .await;

            let url = format!("{}/payload", server.uri());
            let request = make_request(HttpMethod::Get, url);

            let svc = HttpExecutorService::new().expect("executor init");
            let resp = svc
                .execute(&request)
                .await
                .expect("execute should not fail");

            let has_custom = resp
                .headers
                .iter()
                .any(|h| h.key.eq_ignore_ascii_case("x-custom") && h.value == "integrity-check");

            (resp.body, resp.response_size_bytes, has_custom)
        });

        // (a) body content must match
        prop_assert_eq!(
            result_body.as_str(),
            body_text_clone.as_str(),
            "body mismatch: expected {:?}, got {:?}",
            body_text_clone,
            result_body
        );

        // (b) reported size must equal byte length of raw body
        prop_assert_eq!(
            result_size,
            expected_size,
            "size mismatch: expected {}, got {}",
            expected_size,
            result_size
        );

        // (c) response headers must include the custom header injected by mock
        prop_assert!(has_custom, "x-custom header missing from response headers");
    }
}

// ---------------------------------------------------------------------------
// Additional deterministic sanity checks (not property-based)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn executor_returns_correct_status_code_from_mock() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/status"))
        .respond_with(ResponseTemplate::new(201))
        .mount(&server)
        .await;

    let url = format!("{}/status", server.uri());
    let request = make_request(HttpMethod::Get, url);

    let svc = HttpExecutorService::new().unwrap();
    let resp = svc.execute(&request).await.unwrap();

    assert_eq!(resp.status_code, 201);
}

#[tokio::test]
async fn executor_measures_response_time() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/time"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let url = format!("{}/time", server.uri());
    let request = make_request(HttpMethod::Get, url);

    let svc = HttpExecutorService::new().unwrap();
    let resp = svc.execute(&request).await.unwrap();

    // response_time_ms is a u64 so it can't be negative; just assert it's
    // a plausible value (< 60 seconds for a loopback request).
    assert!(resp.response_time_ms < 60_000, "response time implausibly large");
}

#[tokio::test]
async fn executor_captures_response_headers() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/headers"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("x-trace-id", "abc-123")
                .insert_header("content-type", "text/plain"),
        )
        .mount(&server)
        .await;

    let url = format!("{}/headers", server.uri());
    let request = make_request(HttpMethod::Get, url);

    let svc = HttpExecutorService::new().unwrap();
    let resp = svc.execute(&request).await.unwrap();

    let has_trace = resp
        .headers
        .iter()
        .any(|h| h.key.eq_ignore_ascii_case("x-trace-id") && h.value == "abc-123");

    assert!(has_trace, "expected x-trace-id in response headers");
}

#[tokio::test]
async fn executor_propagates_request_headers_to_server() {
    use wiremock::matchers::header;

    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/echo"))
        .and(header("x-client-id", "lightman"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let url = format!("{}/echo", server.uri());
    let mut request = make_request(HttpMethod::Get, url);
    request.headers = vec![Header {
        key: "x-client-id".to_string(),
        value: "lightman".to_string(),
        enabled: true,
    }];

    let svc = HttpExecutorService::new().unwrap();
    let resp = svc.execute(&request).await.unwrap();

    // Mock only responds 200 when the header is present — so 200 confirms it.
    assert_eq!(resp.status_code, 200);
}
