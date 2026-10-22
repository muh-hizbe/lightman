// Domain models — all core data structures used across services and IPC commands.
// Task 3.1: Define all domain structs with full serde/debug/clone derives.
//
// Requirements: 1.1, 1.3, 2.1, 3.1, 4.1, 9.1

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// HTTP Method
// ---------------------------------------------------------------------------

/// Supported HTTP methods (Requirement 1.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
}

impl std::fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
            HttpMethod::Put => "PUT",
            HttpMethod::Patch => "PATCH",
            HttpMethod::Delete => "DELETE",
            HttpMethod::Head => "HEAD",
            HttpMethod::Options => "OPTIONS",
        };
        write!(f, "{s}")
    }
}

// ---------------------------------------------------------------------------
// Header and Query Parameter
// ---------------------------------------------------------------------------

/// A single HTTP header (key-value pair with an enabled toggle).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Header {
    pub key: String,
    pub value: String,
    /// When `false` the header is excluded from the outgoing request.
    pub enabled: bool,
}

/// A single URL query parameter (key-value pair with an enabled toggle).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryParam {
    pub key: String,
    pub value: String,
    /// When `false` the param is excluded from the outgoing request.
    pub enabled: bool,
}

// ---------------------------------------------------------------------------
// Request Body (Requirement 1.3)
// ---------------------------------------------------------------------------

/// Represents one field in a multipart/form-data request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultipartField {
    pub key: String,
    pub value: String,
    pub enabled: bool,
}

/// The body payload attached to an HTTP request.
///
/// Serialisation uses an internally tagged `"type"` field so the stored JSON
/// matches the shape expected by the storage layer (e.g. `{"type":"none"}`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RequestBody {
    /// No body (default).
    None,
    /// Raw JSON string — `Content-Type: application/json`.
    Json { content: String },
    /// URL-encoded key-value pairs — `Content-Type: application/x-www-form-urlencoded`.
    FormUrlEncoded { fields: Vec<MultipartField> },
    /// Multipart/form-data — `Content-Type: multipart/form-data`.
    Multipart { fields: Vec<MultipartField> },
    /// Plain/arbitrary text — `Content-Type: text/plain` (or user-supplied).
    Raw { content: String, content_type: String },
    /// Binary file upload — stores the absolute file path on disk.
    Binary { file_path: String },
}

impl Default for RequestBody {
    fn default() -> Self {
        RequestBody::None
    }
}

// ---------------------------------------------------------------------------
// Authentication Configuration (Requirement 9.1)
// ---------------------------------------------------------------------------

/// Where an API key is injected when using `AuthConfig::ApiKey`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiKeyLocation {
    /// Inject the key as an HTTP request header.
    Header,
    /// Append the key as a URL query parameter.
    QueryParam,
}

/// Authentication scheme to attach to an HTTP request or collection/folder.
///
/// Uses an internally tagged `"type"` field matching the storage JSON shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthConfig {
    /// No authentication.
    None,
    /// Inherit authentication from the parent collection/folder.
    /// Resolved at request-execution time by walking up the `parent_id` chain.
    Inherit,
    /// HTTP Basic authentication — credentials are base64-encoded by the Header Merger.
    BasicAuth {
        username: String,
        password: String,
    },
    /// Bearer token — injected as `Authorization: Bearer <token>`.
    BearerToken { token: String },
    /// API key — injected either as a header or a query parameter.
    ApiKey {
        key: String,
        value: String,
        location: ApiKeyLocation,
    },
}

impl Default for AuthConfig {
    fn default() -> Self {
        AuthConfig::Inherit
    }
}

// ---------------------------------------------------------------------------
// Collection (Requirement 2.1)
// ---------------------------------------------------------------------------

/// A named container for HTTP requests; can also act as a folder when
/// `parent_id` is set (supports arbitrary nesting).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Collection {
    /// UUID v4 identifier.
    pub id: String,
    /// Display name — 1–255 characters.
    pub name: String,
    /// Optional free-text description.
    pub description: Option<String>,
    /// Parent collection ID when this is a nested folder; `None` for top-level.
    pub parent_id: Option<String>,
    /// Zero-based ordering index within the parent.
    pub position: i64,
    /// ISO-8601 creation timestamp.
    pub created_at: String,
    /// ISO-8601 last-update timestamp.
    pub updated_at: String,
    /// Default headers applied to every request in this collection.
    /// Overridable per-request.
    pub default_headers: Vec<Header>,
    /// Authentication configuration for this collection/folder.
    /// Requests with `AuthConfig::Inherit` will use this value (walking up
    /// the hierarchy if this is also `Inherit`).
    pub auth: AuthConfig,
}

// ---------------------------------------------------------------------------
// Request (Requirements 1.1, 1.3, 2.1)
// ---------------------------------------------------------------------------

/// A complete HTTP request configuration stored inside a `Collection`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    /// UUID v4 identifier.
    pub id: String,
    /// ID of the owning `Collection` (or folder).
    pub collection_id: String,
    /// Display name — 1–255 characters.
    pub name: String,
    /// HTTP method.
    pub method: HttpMethod,
    /// Request URL — may contain `{{variable}}` placeholders.
    pub url: String,
    /// Request-level headers.
    pub headers: Vec<Header>,
    /// URL query parameters.
    pub params: Vec<QueryParam>,
    /// Request body.
    pub body: RequestBody,
    /// Authentication configuration.
    pub auth: AuthConfig,
    /// Zero-based ordering index within the parent collection.
    pub position: i64,
    /// ISO-8601 creation timestamp.
    pub created_at: String,
    /// ISO-8601 last-update timestamp.
    pub updated_at: String,
}

// ---------------------------------------------------------------------------
// Environment and Variable (Requirement 3.1)
// ---------------------------------------------------------------------------

/// A named set of variables that can be activated globally.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Environment {
    /// UUID v4 identifier.
    pub id: String,
    /// Display name — 1–255 characters.
    pub name: String,
    /// `true` when this environment is currently active.
    pub is_active: bool,
    /// ISO-8601 creation timestamp.
    pub created_at: String,
    /// ISO-8601 last-update timestamp.
    pub updated_at: String,
}

/// A key-value variable entry belonging to an `Environment` (or global when
/// `environment_id` is `None`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Variable {
    /// UUID v4 identifier.
    pub id: String,
    /// Owning environment ID; `None` for global variables.
    pub environment_id: Option<String>,
    /// Variable name used in `{{key}}` placeholders.
    pub key: String,
    /// Resolved value.
    pub value: String,
    /// When `false` the variable is excluded from interpolation.
    pub enabled: bool,
}

// ---------------------------------------------------------------------------
// HTTP Response (Requirement 1.5)
// ---------------------------------------------------------------------------

/// The result of an HTTP request execution returned to the frontend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HttpResponse {
    /// HTTP status code (e.g. 200, 404).
    pub status_code: u16,
    /// Status text (e.g. "OK", "Not Found").
    pub status_text: String,
    /// Response headers as key-value pairs.
    pub headers: Vec<Header>,
    /// Response body as a UTF-8 string (binary responses are base64-encoded).
    pub body: String,
    /// Total round-trip time in milliseconds.
    pub response_time_ms: u64,
    /// Size of the response body in bytes.
    pub response_size_bytes: u64,
}

// ---------------------------------------------------------------------------
// History Entry (Requirement 4.1)
// ---------------------------------------------------------------------------

/// A record of one HTTP request execution, including request config and
/// response metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// UUID v4 identifier.
    pub id: String,
    /// Full request configuration at the time of execution.
    pub request: Request,
    /// HTTP status code from the response.
    pub status_code: u16,
    /// Status text (e.g. "OK", "Not Found").
    pub status_text: String,
    /// Round-trip time in milliseconds.
    pub response_time_ms: u64,
    /// Response body size in bytes.
    pub response_size_bytes: u64,
    /// Response headers.
    pub response_headers: Vec<Header>,
    /// Response body text.
    pub response_body: String,
    /// ISO-8601 timestamp of when the request was executed.
    pub executed_at: String,
}

// ---------------------------------------------------------------------------
// Saved Response (Feature: save response per request)
// ---------------------------------------------------------------------------

/// A named snapshot of an HTTP response, associated with a specific `Request`.
/// Multiple snapshots can be saved per request and renamed by the user.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedResponse {
    /// UUID v4 identifier.
    pub id: String,
    /// ID of the owning `Request`.
    pub request_id: String,
    /// User-provided label (defaults to "Response <n>" when first saved).
    pub label: String,
    /// HTTP status code (e.g. 200, 404).
    pub status_code: u16,
    /// Status text (e.g. "OK", "Not Found").
    pub status_text: String,
    /// Response headers.
    pub headers: Vec<Header>,
    /// Response body (UTF-8 text or base64 for binary).
    pub body: String,
    /// Round-trip time in milliseconds.
    pub response_time_ms: u64,
    /// Response size in bytes.
    pub response_size_bytes: u64,
    /// ISO-8601 timestamp of when this response was saved.
    pub saved_at: String,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    // --- HttpMethod ---

    #[test]
    fn http_method_display() {
        assert_eq!(HttpMethod::Get.to_string(), "GET");
        assert_eq!(HttpMethod::Post.to_string(), "POST");
        assert_eq!(HttpMethod::Put.to_string(), "PUT");
        assert_eq!(HttpMethod::Patch.to_string(), "PATCH");
        assert_eq!(HttpMethod::Delete.to_string(), "DELETE");
        assert_eq!(HttpMethod::Head.to_string(), "HEAD");
        assert_eq!(HttpMethod::Options.to_string(), "OPTIONS");
    }

    #[test]
    fn http_method_serde_round_trip() {
        let methods = [
            HttpMethod::Get,
            HttpMethod::Post,
            HttpMethod::Put,
            HttpMethod::Patch,
            HttpMethod::Delete,
            HttpMethod::Head,
            HttpMethod::Options,
        ];
        for method in &methods {
            let json = serde_json::to_string(method).unwrap();
            let decoded: HttpMethod = serde_json::from_str(&json).unwrap();
            assert_eq!(method, &decoded);
        }
    }

    // --- RequestBody ---

    #[test]
    fn request_body_none_serialises_with_type_tag() {
        let body = RequestBody::None;
        let json = serde_json::to_string(&body).unwrap();
        assert!(json.contains("\"type\":\"none\""), "got: {json}");
    }

    #[test]
    fn request_body_json_variant_round_trip() {
        let body = RequestBody::Json {
            content: r#"{"key":"value"}"#.to_string(),
        };
        let json = serde_json::to_string(&body).unwrap();
        let decoded: RequestBody = serde_json::from_str(&json).unwrap();
        assert_eq!(body, decoded);
    }

    #[test]
    fn request_body_form_url_encoded_round_trip() {
        let body = RequestBody::FormUrlEncoded {
            fields: vec![MultipartField {
                key: "user".to_string(),
                value: "alice".to_string(),
                enabled: true,
            }],
        };
        let json = serde_json::to_string(&body).unwrap();
        let decoded: RequestBody = serde_json::from_str(&json).unwrap();
        assert_eq!(body, decoded);
    }

    #[test]
    fn request_body_multipart_round_trip() {
        let body = RequestBody::Multipart {
            fields: vec![MultipartField {
                key: "file".to_string(),
                value: "data".to_string(),
                enabled: true,
            }],
        };
        let json = serde_json::to_string(&body).unwrap();
        let decoded: RequestBody = serde_json::from_str(&json).unwrap();
        assert_eq!(body, decoded);
    }

    #[test]
    fn request_body_raw_round_trip() {
        let body = RequestBody::Raw {
            content: "hello world".to_string(),
            content_type: "text/plain".to_string(),
        };
        let json = serde_json::to_string(&body).unwrap();
        let decoded: RequestBody = serde_json::from_str(&json).unwrap();
        assert_eq!(body, decoded);
    }

    #[test]
    fn request_body_binary_round_trip() {
        let body = RequestBody::Binary {
            file_path: "/tmp/upload.bin".to_string(),
        };
        let json = serde_json::to_string(&body).unwrap();
        let decoded: RequestBody = serde_json::from_str(&json).unwrap();
        assert_eq!(body, decoded);
    }

    // --- AuthConfig ---

    #[test]
    fn auth_config_none_serialises_with_type_tag() {
        let auth = AuthConfig::None;
        let json = serde_json::to_string(&auth).unwrap();
        assert!(json.contains("\"type\":\"none\""), "got: {json}");
    }

    #[test]
    fn auth_config_basic_round_trip() {
        let auth = AuthConfig::BasicAuth {
            username: "admin".to_string(),
            password: "secret".to_string(),
        };
        let json = serde_json::to_string(&auth).unwrap();
        let decoded: AuthConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(auth, decoded);
    }

    #[test]
    fn auth_config_bearer_round_trip() {
        let auth = AuthConfig::BearerToken {
            token: "tok_abc123".to_string(),
        };
        let json = serde_json::to_string(&auth).unwrap();
        let decoded: AuthConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(auth, decoded);
    }

    #[test]
    fn auth_config_api_key_header_round_trip() {
        let auth = AuthConfig::ApiKey {
            key: "X-Api-Key".to_string(),
            value: "my-key".to_string(),
            location: ApiKeyLocation::Header,
        };
        let json = serde_json::to_string(&auth).unwrap();
        let decoded: AuthConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(auth, decoded);
    }

    #[test]
    fn auth_config_api_key_query_param_round_trip() {
        let auth = AuthConfig::ApiKey {
            key: "apiKey".to_string(),
            value: "12345".to_string(),
            location: ApiKeyLocation::QueryParam,
        };
        let json = serde_json::to_string(&auth).unwrap();
        let decoded: AuthConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(auth, decoded);
    }

    // --- Collection ---

    #[test]
    fn collection_serde_round_trip() {
        let col = Collection {
            id: "col-1".to_string(),
            name: "My API".to_string(),
            description: Some("Test collection".to_string()),
            parent_id: None,
            position: 0,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
            default_headers: vec![Header {
                key: "Accept".to_string(),
                value: "application/json".to_string(),
                enabled: true,
            }],
            auth: AuthConfig::None,
        };
        let json = serde_json::to_string(&col).unwrap();
        let decoded: Collection = serde_json::from_str(&json).unwrap();
        assert_eq!(col, decoded);
    }

    // --- Request ---

    #[test]
    fn request_serde_round_trip() {
        let req = Request {
            id: "req-1".to_string(),
            collection_id: "col-1".to_string(),
            name: "Get todos".to_string(),
            method: HttpMethod::Get,
            url: "https://jsonplaceholder.typicode.com/todos/{{id}}".to_string(),
            headers: vec![Header {
                key: "Accept".to_string(),
                value: "application/json".to_string(),
                enabled: true,
            }],
            params: vec![QueryParam {
                key: "_limit".to_string(),
                value: "10".to_string(),
                enabled: true,
            }],
            body: RequestBody::None,
            auth: AuthConfig::None,
            position: 0,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        let decoded: Request = serde_json::from_str(&json).unwrap();
        assert_eq!(req, decoded);
    }

    // --- Environment + Variable ---

    #[test]
    fn environment_serde_round_trip() {
        let env = Environment {
            id: "env-1".to_string(),
            name: "Development".to_string(),
            is_active: true,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&env).unwrap();
        let decoded: Environment = serde_json::from_str(&json).unwrap();
        assert_eq!(env, decoded);
    }

    #[test]
    fn variable_global_serde_round_trip() {
        let var = Variable {
            id: "var-1".to_string(),
            environment_id: None, // global variable
            key: "BASE_URL".to_string(),
            value: "https://api.example.com".to_string(),
            enabled: true,
        };
        let json = serde_json::to_string(&var).unwrap();
        let decoded: Variable = serde_json::from_str(&json).unwrap();
        assert_eq!(var, decoded);
    }

    #[test]
    fn variable_env_scoped_serde_round_trip() {
        let var = Variable {
            id: "var-2".to_string(),
            environment_id: Some("env-1".to_string()),
            key: "API_KEY".to_string(),
            value: "secret".to_string(),
            enabled: true,
        };
        let json = serde_json::to_string(&var).unwrap();
        let decoded: Variable = serde_json::from_str(&json).unwrap();
        assert_eq!(var, decoded);
    }

    // --- HttpResponse ---

    #[test]
    fn http_response_serde_round_trip() {
        let resp = HttpResponse {
            status_code: 200,
            status_text: "OK".to_string(),
            headers: vec![Header {
                key: "Content-Type".to_string(),
                value: "application/json".to_string(),
                enabled: true,
            }],
            body: r#"{"id":1}"#.to_string(),
            response_time_ms: 42,
            response_size_bytes: 8,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let decoded: HttpResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(resp, decoded);
    }

    // --- HistoryEntry ---

    #[test]
    fn history_entry_serde_round_trip() {
        let req = Request {
            id: "req-1".to_string(),
            collection_id: "col-1".to_string(),
            name: "ping".to_string(),
            method: HttpMethod::Get,
            url: "https://example.com".to_string(),
            headers: vec![],
            params: vec![],
            body: RequestBody::None,
            auth: AuthConfig::None,
            position: 0,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        };
        let entry = HistoryEntry {
            id: "hist-1".to_string(),
            request: req,
            status_code: 200,
            status_text: "OK".to_string(),
            response_time_ms: 100,
            response_size_bytes: 512,
            response_headers: vec![],
            response_body: "pong".to_string(),
            executed_at: "2024-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let decoded: HistoryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry, decoded);
    }
}
