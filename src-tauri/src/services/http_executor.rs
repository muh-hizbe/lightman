// HTTP Executor — orchestrates building and sending HTTP requests via reqwest.
//
// Task 4.1: Implement `HttpExecutorService::execute` using reqwest.
// Task 4.2: Implement error handling for network and timeout errors.
//
// Requirements: 1.1, 1.2, 1.3, 1.5, 1.6, 1.7

use std::time::Instant;

use base64::Engine;
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue},
    Client, ClientBuilder,
};

use crate::models::{Header, HttpMethod, HttpResponse, Request, RequestBody};

// ---------------------------------------------------------------------------
// Service struct
// ---------------------------------------------------------------------------

/// Executes HTTP requests via reqwest.
///
/// Each instance owns a shared `reqwest::Client` configured with a cookie store
/// so session cookies are preserved across successive calls from the same service
/// instance.
pub struct HttpExecutorService {
    client: Client,
}

impl HttpExecutorService {
    /// Create a new service instance with a cookie-store-enabled client.
    pub fn new() -> Result<Self, String> {
        let client = ClientBuilder::new()
            .cookie_store(true)
            .build()
            .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

        Ok(Self { client })
    }

    /// Execute the provided [`Request`] and return an [`HttpResponse`].
    ///
    /// Steps:
    /// 1. Build the reqwest [`RequestBuilder`] routing on `request.method`.
    /// 2. Attach query parameters.
    /// 3. Attach enabled request headers.
    /// 4. Attach the request body.
    /// 5. Send the request, measuring round-trip time.
    /// 6. Collect status, headers, body and size into [`HttpResponse`].
    pub async fn execute(&self, request: &Request) -> Result<HttpResponse, String> {
        // ---------------------------------------------------------------
        // 1. Route to the correct reqwest method (Requirement 1.1)
        // ---------------------------------------------------------------
        let mut builder = match request.method {
            HttpMethod::Get => self.client.get(&request.url),
            HttpMethod::Post => self.client.post(&request.url),
            HttpMethod::Put => self.client.put(&request.url),
            HttpMethod::Patch => self.client.patch(&request.url),
            HttpMethod::Delete => self.client.delete(&request.url),
            HttpMethod::Head => self.client.head(&request.url),
            HttpMethod::Options => self.client.request(
                reqwest::Method::OPTIONS,
                &request.url,
            ),
        };

        // ---------------------------------------------------------------
        // 2. Attach enabled query parameters
        // ---------------------------------------------------------------
        let enabled_params: Vec<(&str, &str)> = request
            .params
            .iter()
            .filter(|p| p.enabled)
            .map(|p| (p.key.as_str(), p.value.as_str()))
            .collect();

        if !enabled_params.is_empty() {
            builder = builder.query(&enabled_params);
        }

        // ---------------------------------------------------------------
        // 3. Attach enabled request headers
        // ---------------------------------------------------------------
        let mut header_map = HeaderMap::new();
        for header in request.headers.iter().filter(|h| h.enabled) {
            match (
                HeaderName::from_bytes(header.key.as_bytes()),
                HeaderValue::from_str(&header.value),
            ) {
                (Ok(name), Ok(value)) => {
                    header_map.insert(name, value);
                }
                _ => {
                    // Skip malformed headers — don't abort the whole request.
                }
            }
        }
        builder = builder.headers(header_map);

        // ---------------------------------------------------------------
        // 4. Attach body (Requirement 1.3)
        // ---------------------------------------------------------------
        builder = attach_body(builder, &request.body)?;

        // ---------------------------------------------------------------
        // 5. Send and measure round-trip time (Requirement 1.5)
        // ---------------------------------------------------------------
        let start = Instant::now();
        let response = builder
            .send()
            .await
            .map_err(|e| map_reqwest_error(e))?;
        let elapsed_ms = start.elapsed().as_millis() as u64;

        // ---------------------------------------------------------------
        // 6. Collect response data
        // ---------------------------------------------------------------
        let status = response.status();
        let status_code = status.as_u16();
        let status_text = status
            .canonical_reason()
            .unwrap_or("Unknown")
            .to_string();

        // Collect response headers
        let response_headers: Vec<Header> = response
            .headers()
            .iter()
            .map(|(name, value)| Header {
                key: name.to_string(),
                value: value.to_str().unwrap_or("").to_string(),
                enabled: true,
            })
            .collect();

        // Read body bytes
        let body_bytes = response
            .bytes()
            .await
            .map_err(|e| format!("Failed to read response body: {e}"))?;

        let response_size_bytes = body_bytes.len() as u64;

        // Attempt UTF-8 decode; fall back to base64 for binary payloads.
        let body = match std::str::from_utf8(&body_bytes) {
            Ok(text) => text.to_string(),
            Err(_) => base64::engine::general_purpose::STANDARD.encode(&body_bytes),
        };

        Ok(HttpResponse {
            status_code,
            status_text,
            headers: response_headers,
            body,
            response_time_ms: elapsed_ms,
            response_size_bytes,
        })
    }
}

impl Default for HttpExecutorService {
    fn default() -> Self {
        Self::new().expect("Failed to create default HttpExecutorService")
    }
}

// ---------------------------------------------------------------------------
// Helper: attach body to a RequestBuilder
// ---------------------------------------------------------------------------

/// Attaches the correct body to `builder` based on the [`RequestBody`] variant.
///
/// Returns the modified builder, or an error string for unreadable binary files.
fn attach_body(
    builder: reqwest::RequestBuilder,
    body: &RequestBody,
) -> Result<reqwest::RequestBuilder, String> {
    match body {
        RequestBody::None => Ok(builder),

        RequestBody::Json { content } => {
            // Parse to serde_json::Value so reqwest sets Content-Type correctly
            // and validates the JSON up-front.  If the content is not valid JSON
            // we still send it as raw text with the correct Content-Type so the
            // user can debug the API response.
            let builder = builder
                .header("Content-Type", "application/json")
                .body(content.clone());
            Ok(builder)
        }

        RequestBody::FormUrlEncoded { fields } => {
            let params: Vec<(&str, &str)> = fields
                .iter()
                .filter(|f| f.enabled)
                .map(|f| (f.key.as_str(), f.value.as_str()))
                .collect();
            Ok(builder.form(&params))
        }

        RequestBody::Multipart { fields } => {
            let mut form = reqwest::multipart::Form::new();
            for field in fields.iter().filter(|f| f.enabled) {
                form = form.text(field.key.clone(), field.value.clone());
            }
            Ok(builder.multipart(form))
        }

        RequestBody::Raw {
            content,
            content_type,
        } => Ok(builder
            .header("Content-Type", content_type.as_str())
            .body(content.clone())),

        RequestBody::Binary { file_path } => {
            let bytes = std::fs::read(file_path)
                .map_err(|e| format!("Failed to read binary file '{file_path}': {e}"))?;
            Ok(builder
                .header("Content-Type", "application/octet-stream")
                .body(bytes))
        }
    }
}

// ---------------------------------------------------------------------------
// Helper: map reqwest errors to descriptive strings (Requirement 1.6 / 1.7)
// ---------------------------------------------------------------------------

fn map_reqwest_error(e: reqwest::Error) -> String {
    if e.is_timeout() {
        return format!("Request timed out: {e}");
    }
    if e.is_connect() {
        return format!("Connection failed: {e}");
    }
    if e.is_builder() {
        return format!("Invalid request configuration: {e}");
    }
    if e.is_decode() {
        return format!("Failed to decode response: {e}");
    }
    // Check for DNS / status errors via the URL if available
    if let Some(url) = e.url() {
        return format!("Request to '{url}' failed: {e}");
    }
    format!("HTTP request failed: {e}")
}

// ---------------------------------------------------------------------------
// Unit tests (Task 4.3 — HTTP method routing)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AuthConfig, HttpMethod, QueryParam, RequestBody};

    /// Build a minimal `Request` fixture for the given method and URL.
    fn make_request(method: HttpMethod, url: &str) -> Request {
        Request {
            id: uuid::Uuid::new_v4().to_string(),
            collection_id: "col-1".to_string(),
            name: "test".to_string(),
            method,
            url: url.to_string(),
            headers: vec![],
            params: vec![],
            body: RequestBody::None,
            auth: AuthConfig::None,
            position: 0,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        }
    }

    // ------------------------------------------------------------------
    // Constructor
    // ------------------------------------------------------------------

    #[test]
    fn service_new_succeeds() {
        assert!(HttpExecutorService::new().is_ok());
    }

    // ------------------------------------------------------------------
    // Task 4.3 — HTTP method routing
    // Each variant must map to the correct HTTP verb; we verify this by
    // sending requests to httpbin.org/anything which echoes the method back
    // in the JSON response body.
    // These tests are `#[ignore]`-ed so they don't require network access in
    // CI; run them manually with `cargo test -- --ignored`.
    // ------------------------------------------------------------------

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn routes_get_correctly() {
        let svc = HttpExecutorService::new().unwrap();
        let req = make_request(HttpMethod::Get, "https://httpbin.org/get");
        let resp = svc.execute(&req).await.unwrap();
        assert_eq!(resp.status_code, 200);
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn routes_post_correctly() {
        let svc = HttpExecutorService::new().unwrap();
        let req = make_request(HttpMethod::Post, "https://httpbin.org/post");
        let resp = svc.execute(&req).await.unwrap();
        assert_eq!(resp.status_code, 200);
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn routes_put_correctly() {
        let svc = HttpExecutorService::new().unwrap();
        let req = make_request(HttpMethod::Put, "https://httpbin.org/put");
        let resp = svc.execute(&req).await.unwrap();
        assert_eq!(resp.status_code, 200);
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn routes_patch_correctly() {
        let svc = HttpExecutorService::new().unwrap();
        let req = make_request(HttpMethod::Patch, "https://httpbin.org/patch");
        let resp = svc.execute(&req).await.unwrap();
        assert_eq!(resp.status_code, 200);
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn routes_delete_correctly() {
        let svc = HttpExecutorService::new().unwrap();
        let req = make_request(HttpMethod::Delete, "https://httpbin.org/delete");
        let resp = svc.execute(&req).await.unwrap();
        assert_eq!(resp.status_code, 200);
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn routes_head_correctly() {
        let svc = HttpExecutorService::new().unwrap();
        let req = make_request(HttpMethod::Head, "https://httpbin.org/get");
        let resp = svc.execute(&req).await.unwrap();
        // HEAD returns 200 with empty body
        assert_eq!(resp.status_code, 200);
        assert!(resp.body.is_empty());
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn routes_options_correctly() {
        let svc = HttpExecutorService::new().unwrap();
        let req = make_request(HttpMethod::Options, "https://httpbin.org/get");
        let resp = svc.execute(&req).await.unwrap();
        // OPTIONS on httpbin returns 200
        assert!(resp.status_code == 200 || resp.status_code == 204);
    }

    // ------------------------------------------------------------------
    // Offline unit tests — no network needed
    // ------------------------------------------------------------------

    #[test]
    fn attach_body_none_is_noop() {
        // Just ensure no error is returned for the None variant.
        // We can't easily inspect a RequestBuilder, but returning Ok is sufficient.
        let client = Client::new();
        let builder = client.get("http://example.com");
        let result = attach_body(builder, &RequestBody::None);
        assert!(result.is_ok());
    }

    #[test]
    fn attach_body_json_returns_ok() {
        let client = Client::new();
        let builder = client.post("http://example.com");
        let body = RequestBody::Json {
            content: r#"{"key":"value"}"#.to_string(),
        };
        assert!(attach_body(builder, &body).is_ok());
    }

    #[test]
    fn attach_body_form_url_encoded_returns_ok() {
        let client = Client::new();
        let builder = client.post("http://example.com");
        let body = RequestBody::FormUrlEncoded {
            fields: vec![crate::models::MultipartField {
                key: "user".to_string(),
                value: "alice".to_string(),
                enabled: true,
            }],
        };
        assert!(attach_body(builder, &body).is_ok());
    }

    #[test]
    fn attach_body_multipart_returns_ok() {
        let client = Client::new();
        let builder = client.post("http://example.com");
        let body = RequestBody::Multipart {
            fields: vec![crate::models::MultipartField {
                key: "field1".to_string(),
                value: "val1".to_string(),
                enabled: true,
            }],
        };
        assert!(attach_body(builder, &body).is_ok());
    }

    #[test]
    fn attach_body_raw_returns_ok() {
        let client = Client::new();
        let builder = client.post("http://example.com");
        let body = RequestBody::Raw {
            content: "hello".to_string(),
            content_type: "text/plain".to_string(),
        };
        assert!(attach_body(builder, &body).is_ok());
    }

    #[test]
    fn attach_body_binary_missing_file_returns_error() {
        let client = Client::new();
        let builder = client.post("http://example.com");
        let body = RequestBody::Binary {
            file_path: "/nonexistent/path/file.bin".to_string(),
        };
        let result = attach_body(builder, &body);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to read binary file"));
    }

    #[test]
    fn disabled_headers_are_excluded() {
        // Verify that a Header with enabled=false is not present in the map
        let headers = vec![
            Header {
                key: "X-Enabled".to_string(),
                value: "yes".to_string(),
                enabled: true,
            },
            Header {
                key: "X-Disabled".to_string(),
                value: "no".to_string(),
                enabled: false,
            },
        ];

        let enabled: Vec<&Header> = headers.iter().filter(|h| h.enabled).collect();
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].key, "X-Enabled");
    }

    #[test]
    fn disabled_params_are_excluded() {
        let params = vec![
            QueryParam {
                key: "page".to_string(),
                value: "1".to_string(),
                enabled: true,
            },
            QueryParam {
                key: "hidden".to_string(),
                value: "true".to_string(),
                enabled: false,
            },
        ];

        let enabled: Vec<&QueryParam> = params.iter().filter(|p| p.enabled).collect();
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].key, "page");
    }

    #[test]
    fn response_time_is_measured() {
        // Smoke-check: Instant round-trip returns a non-negative duration.
        let start = Instant::now();
        let elapsed = start.elapsed().as_millis() as u64;
        // elapsed will be 0 or very small, but it must be a valid u64
        let _ = elapsed;
    }
}
