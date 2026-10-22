// Tauri IPC commands for sending HTTP requests.
// Task 9.1: Implement `send_request` — the main pipeline that chains
//           interpolation → header merge → HTTP execution → history persistence.
//
// Requirements: 1.2, 1.4, 1.5, 3.2, 9.1, 4.1

use chrono::Utc;
use tauri::State;
use uuid::Uuid;

use crate::models::{Header, HistoryEntry, HttpResponse, Request};
use crate::services::header_merger::HeaderMergerService;
use crate::services::http_executor::HttpExecutorService;
use crate::services::var_interpolator::VarInterpolatorService;
use crate::storage::DbHandle;
use crate::{services, storage};

// ---------------------------------------------------------------------------
// send_request command
// ---------------------------------------------------------------------------

/// Execute an HTTP request through the full pipeline:
///
/// 1. **Variable interpolation** — resolve `{{variable}}` placeholders in the
///    request using active environment variables, collection variables (none
///    for ad-hoc requests), and global variables.
/// 2. **Header merge** — merge collection default headers with per-request
///    headers and inject authentication headers.
/// 3. **HTTP execution** — send the request via `HttpExecutorService` and
///    collect the response.
/// 4. **History persistence** — save a `HistoryEntry` to SQLite.
/// 5. **Return** the `HttpResponse` to the frontend.
///
/// # Parameters
/// - `request` — The full request configuration from the frontend.
/// - `environment_id` — The ID of the currently active environment (may be
///   `None` when no environment is selected).
///
/// # Frontend call
/// ```ts
/// invoke<HttpResponse>('send_request', { request, environmentId })
/// ```
#[tauri::command]
pub async fn send_request(
    db: State<'_, DbHandle>,
    request: Request,
    environment_id: Option<String>,
) -> Result<HttpResponse, String> {
    let db = db.inner().clone();

    // -----------------------------------------------------------------------
    // 1. Fetch variables for interpolation (Requirements 1.2, 3.2)
    // -----------------------------------------------------------------------

    // Active environment variables (highest priority in the chain).
    let env_vars = match &environment_id {
        Some(env_id) => services::environment::list_variables(db.clone(), env_id.clone()).await?,
        None => vec![],
    };

    // Global variables (lowest priority).
    let global_vars = services::environment::list_global_variables(db.clone()).await?;

    // -----------------------------------------------------------------------
    // 2. Apply variable interpolation (Requirement 3.2)
    // -----------------------------------------------------------------------
    // No collection-level variables are threaded through for now (collection
    // variables live on the Collection struct as default_headers, not as
    // Variable entries — the interpolation scope for collection vars is
    // reserved for a future task).
    let interpolated = VarInterpolatorService::interpolate(request, &env_vars, &[], &global_vars);

    // -----------------------------------------------------------------------
    // 3. Fetch collection ancestry for header defaults + auth inheritance
    // -----------------------------------------------------------------------
    // Walk up the parent_id chain: [direct collection, parent, grandparent, …]
    // The ordering is nearest-first so the header merger picks the closest
    // non-Inherit auth.
    let mut ancestor_collections: Vec<crate::models::Collection> = Vec::new();
    let mut current_id: Option<String> = Some(interpolated.collection_id.clone());
    while let Some(cid) = current_id {
        match services::collection::get_collection(db.clone(), cid).await? {
            Some(col) => {
                current_id = col.parent_id.clone();
                ancestor_collections.push(col);
            }
            None => break,
        }
    }

    // The direct collection's default_headers are used for the merge.
    let collection_default_headers: Vec<Header> = ancestor_collections
        .first()
        .map(|c| c.default_headers.clone())
        .unwrap_or_default();

    // -----------------------------------------------------------------------
    // 4. Merge headers + inject auth (Requirements 9.1–9.6)
    // -----------------------------------------------------------------------
    let merge_result = HeaderMergerService::merge(
        &interpolated,
        &collection_default_headers,
        &ancestor_collections,
    );

    // Build the final request that will be sent, with merged headers applied.
    let mut final_request = interpolated.clone();
    final_request.headers = merge_result.headers;

    // Append any extra query params produced by ApiKey/QueryParam auth.
    for (key, value) in &merge_result.extra_query_params {
        final_request.params.push(crate::models::QueryParam {
            key: key.clone(),
            value: value.clone(),
            enabled: true,
        });
    }

    // -----------------------------------------------------------------------
    // 5. Execute the HTTP request (Requirements 1.1, 1.2, 1.5)
    // -----------------------------------------------------------------------
    let executor = HttpExecutorService::new()?;
    let response = executor.execute(&final_request).await?;

    // -----------------------------------------------------------------------
    // 6. Persist a HistoryEntry (Requirement 4.1)
    // -----------------------------------------------------------------------
    let history_id = Uuid::new_v4().to_string();
    let executed_at = Utc::now().to_rfc3339();

    // Serialise the interpolated (pre-merge) request for history so the user
    // sees the resolved values without the auth token leaking into stored
    // headers unnecessarily.  We serialise `interpolated` (variable-resolved
    // but original headers) rather than `final_request` (merged headers).
    let request_json = serde_json::to_string(&interpolated)
        .map_err(|e| format!("Failed to serialise request for history: {e}"))?;

    let response_headers_json = serde_json::to_string(&response.headers)
        .map_err(|e| format!("Failed to serialise response headers for history: {e}"))?;

    storage::insert_history_entry(
        db.clone(),
        history_id.clone(),
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

    // -----------------------------------------------------------------------
    // 7. Return the response to the frontend (Requirement 1.5)
    // -----------------------------------------------------------------------
    Ok(response)
}

// ---------------------------------------------------------------------------
// Helper: build a HistoryEntry domain model from a completed execution.
// (Exposed for use in integration tests — not a Tauri command.)
// ---------------------------------------------------------------------------

/// Build a [`HistoryEntry`] from the resolved request and its response.
///
/// This is a pure, allocation-cheap helper used by both `send_request` and
/// the integration test suite.
pub fn build_history_entry(
    id: String,
    request: &Request,
    response: &HttpResponse,
    executed_at: String,
) -> HistoryEntry {
    HistoryEntry {
        id,
        request: request.clone(),
        status_code: response.status_code,
        status_text: response.status_text.clone(),
        response_time_ms: response.response_time_ms,
        response_size_bytes: response.response_size_bytes,
        response_headers: response.headers.clone(),
        response_body: response.body.clone(),
        executed_at,
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AuthConfig, HttpMethod, HttpResponse, RequestBody};

    fn make_request(url: &str) -> Request {
        Request {
            id: Uuid::new_v4().to_string(),
            collection_id: "col-1".to_string(),
            name: "test".to_string(),
            method: HttpMethod::Get,
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

    fn make_response(status: u16) -> HttpResponse {
        HttpResponse {
            status_code: status,
            status_text: "OK".to_string(),
            headers: vec![Header {
                key: "Content-Type".to_string(),
                value: "application/json".to_string(),
                enabled: true,
            }],
            body: r#"{"result":"ok"}"#.to_string(),
            response_time_ms: 42,
            response_size_bytes: 15,
        }
    }

    // --- build_history_entry ---

    #[test]
    fn build_history_entry_maps_fields_correctly() {
        let req = make_request("https://example.com");
        let resp = make_response(200);
        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4().to_string();

        let entry = build_history_entry(id.clone(), &req, &resp, now.clone());

        assert_eq!(entry.id, id);
        assert_eq!(entry.request.url, req.url);
        assert_eq!(entry.status_code, 200);
        assert_eq!(entry.response_time_ms, 42);
        assert_eq!(entry.response_size_bytes, 15);
        assert_eq!(entry.response_body, r#"{"result":"ok"}"#);
        assert_eq!(entry.response_headers.len(), 1);
        assert_eq!(entry.executed_at, now);
    }

    #[test]
    fn build_history_entry_preserves_request_url() {
        let req = make_request("https://api.example.com/v2/users");
        let resp = make_response(201);
        let entry = build_history_entry(
            "hist-1".to_string(),
            &req,
            &resp,
            "2024-06-01T12:00:00Z".to_string(),
        );
        assert_eq!(entry.request.url, "https://api.example.com/v2/users");
        assert_eq!(entry.status_code, 201);
    }

    #[test]
    fn build_history_entry_with_empty_response_headers() {
        let req = make_request("https://example.com");
        let mut resp = make_response(204);
        resp.headers = vec![];
        resp.body = String::new();
        resp.response_size_bytes = 0;

        let entry = build_history_entry(
            "hist-2".to_string(),
            &req,
            &resp,
            "2024-06-01T12:00:00Z".to_string(),
        );

        assert!(entry.response_headers.is_empty());
        assert!(entry.response_body.is_empty());
        assert_eq!(entry.response_size_bytes, 0);
    }

    // --- Pipeline smoke test (offline) ---

    #[test]
    fn var_interpolation_resolves_url_before_execute_step() {
        use crate::models::Variable;
        use crate::services::var_interpolator::VarInterpolatorService;

        let mut req = make_request("https://{{BASE_URL}}/api");
        req.headers = vec![];

        let env_var = Variable {
            id: Uuid::new_v4().to_string(),
            environment_id: Some("env-1".to_string()),
            key: "BASE_URL".to_string(),
            value: "api.example.com".to_string(),
            enabled: true,
        };

        let resolved = VarInterpolatorService::interpolate(req, &[env_var], &[], &[]);
        assert_eq!(resolved.url, "https://api.example.com/api");
    }

    #[test]
    fn header_merge_injects_bearer_token() {
        use crate::models::AuthConfig;
        use crate::services::header_merger::HeaderMergerService;

        let mut req = make_request("https://example.com");
        req.auth = AuthConfig::BearerToken {
            token: "secret-token".to_string(),
        };

        let result = HeaderMergerService::merge(&req, &[], &[]);
        let auth_header = result
            .headers
            .iter()
            .find(|h| h.key.eq_ignore_ascii_case("Authorization"));

        assert!(auth_header.is_some());
        assert_eq!(auth_header.unwrap().value, "Bearer secret-token");
    }

    #[test]
    fn api_key_query_param_auth_appended_to_params() {
        use crate::models::{ApiKeyLocation, AuthConfig, QueryParam};
        use crate::services::header_merger::HeaderMergerService;

        let mut req = make_request("https://example.com/api");
        req.auth = AuthConfig::ApiKey {
            key: "apikey".to_string(),
            value: "my-key-123".to_string(),
            location: ApiKeyLocation::QueryParam,
        };

        let merge_result = HeaderMergerService::merge(&req, &[], &[]);

        // The extra query param should be present.
        assert_eq!(merge_result.extra_query_params.len(), 1);
        assert_eq!(
            merge_result.extra_query_params[0],
            ("apikey".to_string(), "my-key-123".to_string())
        );

        // Simulate what send_request does: push to req.params.
        let mut final_req = req.clone();
        for (key, value) in &merge_result.extra_query_params {
            final_req.params.push(QueryParam {
                key: key.clone(),
                value: value.clone(),
                enabled: true,
            });
        }

        assert_eq!(final_req.params.len(), 1);
        assert_eq!(final_req.params[0].key, "apikey");
        assert_eq!(final_req.params[0].value, "my-key-123");
    }
}
