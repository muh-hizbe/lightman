// Header Merger — merges Inherited Headers from Collection → Request,
// then injects authentication headers based on `AuthConfig`.
//
// Task 6.1 — Requirements: 9.2, 9.3, 9.4, 9.5, 9.6

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};

use crate::models::{ApiKeyLocation, AuthConfig, Collection, Header, Request};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// The result of a header merge operation.
///
/// In addition to the final merged headers, the query string may need to be
/// augmented when `AuthConfig::ApiKey { location: QueryParam }` is used.
#[derive(Debug, Clone, PartialEq)]
pub struct MergeResult {
    /// Final headers to attach to the outgoing request.
    pub headers: Vec<Header>,
    /// Extra query parameters injected by auth (key, value).
    /// Currently at most one entry (`ApiKey` with `QueryParam` location).
    pub extra_query_params: Vec<(String, String)>,
}

/// Service that merges collection default headers with per-request headers and
/// injects authentication headers.
pub struct HeaderMergerService;

impl HeaderMergerService {
    /// Merge `collection_default_headers` and `request.headers`, then inject
    /// any headers dictated by the resolved auth.
    ///
    /// Auth resolution order (first non-Inherit, non-None wins):
    /// 1. `request.auth` — if not `Inherit`, use directly
    /// 2. Walk `ancestor_collections` in order (nearest first) looking for a
    ///    non-Inherit auth
    /// 3. Fall back to `AuthConfig::None`
    ///
    /// Merge priority for headers (highest wins for duplicate keys):
    /// 1. Auth-injected headers   — always win
    /// 2. Request-level headers   — override collection defaults
    /// 3. Collection default headers — lowest priority
    ///
    /// Only `enabled` headers from each source are included in the output.
    pub fn merge(
        request: &Request,
        collection_default_headers: &[Header],
        ancestor_collections: &[Collection],
    ) -> MergeResult {
        // ------------------------------------------------------------------
        // Step 0: Resolve auth — walk request → nearest ancestor → None
        // ------------------------------------------------------------------
        let resolved_auth = Self::resolve_auth(&request.auth, ancestor_collections);

        // Step 1: Start with enabled collection defaults.
        let mut merged: Vec<Header> = collection_default_headers
            .iter()
            .filter(|h| h.enabled)
            .cloned()
            .collect();

        // Step 2: Apply enabled request headers (override by key, case-insensitive).
        for req_header in request.headers.iter().filter(|h| h.enabled) {
            if let Some(existing) = merged
                .iter_mut()
                .find(|h| h.key.eq_ignore_ascii_case(&req_header.key))
            {
                existing.value = req_header.value.clone();
            } else {
                merged.push(req_header.clone());
            }
        }

        // Step 3: Inject auth headers (override any existing header with the same key).
        let mut extra_query_params: Vec<(String, String)> = Vec::new();

        match resolved_auth {
            AuthConfig::None | AuthConfig::Inherit => {}

            AuthConfig::BasicAuth { username, password } => {
                let credentials = format!("{username}:{password}");
                let encoded = BASE64.encode(credentials.as_bytes());
                let value = format!("Basic {encoded}");
                upsert_header(&mut merged, "Authorization", &value);
            }

            AuthConfig::BearerToken { token } => {
                let value = format!("Bearer {token}");
                upsert_header(&mut merged, "Authorization", &value);
            }

            AuthConfig::ApiKey {
                key,
                value,
                location,
            } => match location {
                ApiKeyLocation::Header => {
                    upsert_header(&mut merged, &key, &value);
                }
                ApiKeyLocation::QueryParam => {
                    extra_query_params.push((key, value));
                }
            },
        }

        MergeResult {
            headers: merged,
            extra_query_params,
        }
    }

    /// Walk the auth chain: request auth → ancestor collections (nearest first).
    /// Returns the first auth that is neither `Inherit` nor `None`, or `None`
    /// if no concrete auth is found.
    fn resolve_auth(request_auth: &AuthConfig, ancestors: &[Collection]) -> AuthConfig {
        if !matches!(request_auth, AuthConfig::Inherit) {
            return request_auth.clone();
        }
        for col in ancestors {
            match &col.auth {
                AuthConfig::Inherit => continue,
                other => return other.clone(),
            }
        }
        AuthConfig::None
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Insert or update a header in `headers` by key (case-insensitive match).
/// The resulting header is always `enabled = true`.
fn upsert_header(headers: &mut Vec<Header>, key: &str, value: &str) {
    if let Some(existing) = headers
        .iter_mut()
        .find(|h| h.key.eq_ignore_ascii_case(key))
    {
        existing.value = value.to_string();
    } else {
        headers.push(Header {
            key: key.to_string(),
            value: value.to_string(),
            enabled: true,
        });
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AuthConfig, ApiKeyLocation, Collection, Header, HttpMethod, RequestBody};

    fn make_request(auth: AuthConfig, headers: Vec<Header>) -> Request {
        Request {
            id: "req-1".to_string(),
            collection_id: "col-1".to_string(),
            name: "test".to_string(),
            method: HttpMethod::Get,
            url: "https://example.com".to_string(),
            headers,
            params: vec![],
            body: RequestBody::None,
            auth,
            position: 0,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        }
    }

    fn make_collection(id: &str, auth: AuthConfig) -> Collection {
        Collection {
            id: id.to_string(),
            name: "Test Collection".to_string(),
            description: None,
            parent_id: None,
            position: 0,
            default_headers: vec![],
            auth,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        }
    }

    fn header(key: &str, value: &str) -> Header {
        Header {
            key: key.to_string(),
            value: value.to_string(),
            enabled: true,
        }
    }

    fn disabled_header(key: &str, value: &str) -> Header {
        Header {
            key: key.to_string(),
            value: value.to_string(),
            enabled: false,
        }
    }

    // --- Basic merge behaviour ---

    #[test]
    fn collection_defaults_included_when_no_request_headers() {
        let req = make_request(AuthConfig::None, vec![]);
        let defaults = vec![header("Accept", "application/json")];
        let result = HeaderMergerService::merge(&req, &defaults, &[]);
        assert_eq!(result.headers.len(), 1);
        assert_eq!(result.headers[0].key, "Accept");
        assert_eq!(result.headers[0].value, "application/json");
    }

    #[test]
    fn request_header_overrides_collection_default() {
        let req = make_request(
            AuthConfig::None,
            vec![header("Accept", "text/plain")],
        );
        let defaults = vec![header("Accept", "application/json")];
        let result = HeaderMergerService::merge(&req, &defaults, &[]);
        let accept: Vec<_> = result
            .headers
            .iter()
            .filter(|h| h.key.eq_ignore_ascii_case("Accept"))
            .collect();
        assert_eq!(accept.len(), 1);
        assert_eq!(accept[0].value, "text/plain");
    }

    #[test]
    fn non_conflicting_headers_are_all_preserved() {
        let req = make_request(
            AuthConfig::None,
            vec![header("X-Custom", "my-value")],
        );
        let defaults = vec![header("Accept", "application/json")];
        let result = HeaderMergerService::merge(&req, &defaults, &[]);
        assert_eq!(result.headers.len(), 2);
    }

    #[test]
    fn disabled_collection_header_is_excluded() {
        let req = make_request(AuthConfig::None, vec![]);
        let defaults = vec![disabled_header("Accept", "application/json")];
        let result = HeaderMergerService::merge(&req, &defaults, &[]);
        assert!(result.headers.is_empty());
    }

    #[test]
    fn disabled_request_header_is_excluded() {
        let req = make_request(
            AuthConfig::None,
            vec![disabled_header("X-Custom", "value")],
        );
        let result = HeaderMergerService::merge(&req, &[], &[]);
        assert!(result.headers.is_empty());
    }

    // --- Auth: None ---

    #[test]
    fn no_auth_produces_no_extra_headers() {
        let req = make_request(AuthConfig::None, vec![]);
        let result = HeaderMergerService::merge(&req, &[], &[]);
        assert!(result.headers.is_empty());
        assert!(result.extra_query_params.is_empty());
    }

    // --- Auth: BasicAuth ---

    #[test]
    fn basic_auth_produces_correct_authorization_header() {
        let req = make_request(
            AuthConfig::BasicAuth {
                username: "alice".to_string(),
                password: "secret".to_string(),
            },
            vec![],
        );
        let result = HeaderMergerService::merge(&req, &[], &[]);
        let auth_header = result
            .headers
            .iter()
            .find(|h| h.key.eq_ignore_ascii_case("Authorization"))
            .expect("Authorization header missing");
        // "alice:secret" base64-encodes to "YWxpY2U6c2VjcmV0"
        assert_eq!(auth_header.value, "Basic YWxpY2U6c2VjcmV0");
    }

    #[test]
    fn basic_auth_empty_password() {
        let req = make_request(
            AuthConfig::BasicAuth {
                username: "user".to_string(),
                password: "".to_string(),
            },
            vec![],
        );
        let result = HeaderMergerService::merge(&req, &[], &[]);
        let auth_header = result
            .headers
            .iter()
            .find(|h| h.key.eq_ignore_ascii_case("Authorization"))
            .unwrap();
        // "user:" encodes to "dXNlcjo="
        assert_eq!(auth_header.value, "Basic dXNlcjo=");
    }

    #[test]
    fn basic_auth_overrides_manual_authorization_header() {
        let req = make_request(
            AuthConfig::BasicAuth {
                username: "alice".to_string(),
                password: "secret".to_string(),
            },
            vec![header("Authorization", "Bearer old-token")],
        );
        let result = HeaderMergerService::merge(&req, &[], &[]);
        let auth_headers: Vec<_> = result
            .headers
            .iter()
            .filter(|h| h.key.eq_ignore_ascii_case("Authorization"))
            .collect();
        assert_eq!(auth_headers.len(), 1);
        assert!(auth_headers[0].value.starts_with("Basic "));
    }

    // --- Auth: BearerToken ---

    #[test]
    fn bearer_token_produces_correct_authorization_header() {
        let req = make_request(
            AuthConfig::BearerToken {
                token: "tok_abc123".to_string(),
            },
            vec![],
        );
        let result = HeaderMergerService::merge(&req, &[], &[]);
        let auth_header = result
            .headers
            .iter()
            .find(|h| h.key.eq_ignore_ascii_case("Authorization"))
            .expect("Authorization header missing");
        assert_eq!(auth_header.value, "Bearer tok_abc123");
    }

    #[test]
    fn bearer_token_overrides_manual_authorization_header() {
        let req = make_request(
            AuthConfig::BearerToken {
                token: "new-token".to_string(),
            },
            vec![header("Authorization", "Basic old-creds")],
        );
        let result = HeaderMergerService::merge(&req, &[], &[]);
        let auth_headers: Vec<_> = result
            .headers
            .iter()
            .filter(|h| h.key.eq_ignore_ascii_case("Authorization"))
            .collect();
        assert_eq!(auth_headers.len(), 1);
        assert_eq!(auth_headers[0].value, "Bearer new-token");
    }

    // --- Auth: ApiKey as Header ---

    #[test]
    fn api_key_header_location_injects_header() {
        let req = make_request(
            AuthConfig::ApiKey {
                key: "X-Api-Key".to_string(),
                value: "my-secret-key".to_string(),
                location: ApiKeyLocation::Header,
            },
            vec![],
        );
        let result = HeaderMergerService::merge(&req, &[], &[]);
        assert!(result.extra_query_params.is_empty());
        let api_key_header = result
            .headers
            .iter()
            .find(|h| h.key == "X-Api-Key")
            .expect("X-Api-Key header missing");
        assert_eq!(api_key_header.value, "my-secret-key");
    }

    #[test]
    fn api_key_header_overrides_manual_header_with_same_key() {
        let req = make_request(
            AuthConfig::ApiKey {
                key: "X-Api-Key".to_string(),
                value: "new-key".to_string(),
                location: ApiKeyLocation::Header,
            },
            vec![header("X-Api-Key", "old-key")],
        );
        let result = HeaderMergerService::merge(&req, &[], &[]);
        let api_key_headers: Vec<_> = result
            .headers
            .iter()
            .filter(|h| h.key.eq_ignore_ascii_case("X-Api-Key"))
            .collect();
        assert_eq!(api_key_headers.len(), 1);
        assert_eq!(api_key_headers[0].value, "new-key");
    }

    // --- Auth: ApiKey as QueryParam ---

    #[test]
    fn api_key_query_param_location_adds_to_extra_query_params() {
        let req = make_request(
            AuthConfig::ApiKey {
                key: "apikey".to_string(),
                value: "12345".to_string(),
                location: ApiKeyLocation::QueryParam,
            },
            vec![],
        );
        let result = HeaderMergerService::merge(&req, &[], &[]);
        assert!(result.headers.is_empty());
        assert_eq!(result.extra_query_params.len(), 1);
        assert_eq!(result.extra_query_params[0], ("apikey".to_string(), "12345".to_string()));
    }

    #[test]
    fn api_key_query_param_does_not_add_any_header() {
        let req = make_request(
            AuthConfig::ApiKey {
                key: "token".to_string(),
                value: "abc".to_string(),
                location: ApiKeyLocation::QueryParam,
            },
            vec![header("Accept", "application/json")],
        );
        let result = HeaderMergerService::merge(&req, &[], &[]);
        assert!(!result
            .headers
            .iter()
            .any(|h| h.key.eq_ignore_ascii_case("Authorization")));
        assert_eq!(result.extra_query_params.len(), 1);
    }

    // --- Case-insensitive key matching ---

    #[test]
    fn header_key_matching_is_case_insensitive() {
        let req = make_request(
            AuthConfig::None,
            vec![header("content-type", "application/json")],
        );
        let defaults = vec![header("Content-Type", "text/html")];
        let result = HeaderMergerService::merge(&req, &defaults, &[]);
        let ct: Vec<_> = result
            .headers
            .iter()
            .filter(|h| h.key.eq_ignore_ascii_case("content-type"))
            .collect();
        assert_eq!(ct.len(), 1);
        assert_eq!(ct[0].value, "application/json");
    }

    // --- Auth inheritance ---

    #[test]
    fn inherit_auth_resolves_from_nearest_ancestor() {
        let req = make_request(AuthConfig::Inherit, vec![]);
        let col = make_collection(
            "col-1",
            AuthConfig::BearerToken { token: "col-token".to_string() },
        );
        let result = HeaderMergerService::merge(&req, &[], &[col]);
        let auth_header = result
            .headers
            .iter()
            .find(|h| h.key.eq_ignore_ascii_case("Authorization"))
            .expect("Authorization header missing");
        assert_eq!(auth_header.value, "Bearer col-token");
    }

    #[test]
    fn inherit_auth_walks_up_chain_skipping_inherit() {
        let req = make_request(AuthConfig::Inherit, vec![]);
        let child_col = make_collection("child", AuthConfig::Inherit);
        let parent_col = make_collection(
            "parent",
            AuthConfig::BasicAuth {
                username: "u".to_string(),
                password: "p".to_string(),
            },
        );
        // nearest ancestor first — child (Inherit) → parent (BasicAuth)
        let result = HeaderMergerService::merge(&req, &[], &[child_col, parent_col]);
        let auth_header = result
            .headers
            .iter()
            .find(|h| h.key.eq_ignore_ascii_case("Authorization"))
            .expect("Authorization header missing");
        assert!(auth_header.value.starts_with("Basic "));
    }

    #[test]
    fn inherit_auth_falls_back_to_none_when_no_ancestor_has_auth() {
        let req = make_request(AuthConfig::Inherit, vec![]);
        let col = make_collection("col-1", AuthConfig::Inherit);
        let result = HeaderMergerService::merge(&req, &[], &[col]);
        assert!(result.headers.is_empty());
        assert!(result.extra_query_params.is_empty());
    }

    // --- No extra_query_params when not using QueryParam ApiKey ---

    #[test]
    fn no_extra_query_params_for_basic_auth() {
        let req = make_request(
            AuthConfig::BasicAuth {
                username: "u".to_string(),
                password: "p".to_string(),
            },
            vec![],
        );
        let result = HeaderMergerService::merge(&req, &[], &[]);
        assert!(result.extra_query_params.is_empty());
    }

    #[test]
    fn no_extra_query_params_for_bearer_auth() {
        let req = make_request(
            AuthConfig::BearerToken {
                token: "t".to_string(),
            },
            vec![],
        );
        let result = HeaderMergerService::merge(&req, &[], &[]);
        assert!(result.extra_query_params.is_empty());
    }
}
