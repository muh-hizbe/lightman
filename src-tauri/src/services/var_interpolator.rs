// Variable Interpolator — replaces {{variable_name}} references with actual values.
// Priority chain: Environment Variable > Collection Variable > Global Variable.
//
// Task 5.1 — Requirements: 3.2, 3.4, 3.5

use crate::models::{Request, RequestBody, Variable};
// Header and QueryParam are only used in tests; imported there directly.
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Resolves `{{key}}` placeholders inside a `Request`.
///
/// Variable scopes (highest priority first):
/// 1. `env_vars`        — active environment variables
/// 2. `collection_vars` — collection-level variables
/// 3. `global_vars`     — global variables (environment_id = NULL)
///
/// Placeholders that cannot be resolved in any scope are left **unchanged**
/// (Requirement 3.5).
///
/// Interpolation is applied to:
/// - `request.url`
/// - Enabled `request.headers` values
/// - Enabled `request.params` values
/// - Body text fields (`Json`, `Raw` content; `FormUrlEncoded` and `Multipart`
///   field values)
pub struct VarInterpolatorService;

impl VarInterpolatorService {
    /// Interpolate all variable placeholders in `request` and return the
    /// resolved `Request`.  The original is consumed to avoid unnecessary
    /// cloning at call sites.
    pub fn interpolate(
        mut request: Request,
        env_vars: &[Variable],
        collection_vars: &[Variable],
        global_vars: &[Variable],
    ) -> Request {
        // Build a flat lookup map respecting the priority chain.
        let lookup = build_lookup(env_vars, collection_vars, global_vars);

        // --- URL ---
        request.url = replace_placeholders(&request.url, &lookup);

        // --- Headers (enabled only, values only) ---
        for header in &mut request.headers {
            if header.enabled {
                header.value = replace_placeholders(&header.value, &lookup);
            }
        }

        // --- Query params (enabled only, values only) ---
        for param in &mut request.params {
            if param.enabled {
                param.value = replace_placeholders(&param.value, &lookup);
            }
        }

        // --- Body ---
        request.body = interpolate_body(request.body, &lookup);

        request
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a `HashMap<key, value>` with the correct priority merge:
/// env_vars override collection_vars which override global_vars.
/// Only enabled variables are included.
fn build_lookup<'a>(
    env_vars: &'a [Variable],
    collection_vars: &'a [Variable],
    global_vars: &'a [Variable],
) -> HashMap<&'a str, &'a str> {
    let mut map: HashMap<&str, &str> = HashMap::new();

    // Lowest priority first so higher-priority layers overwrite.
    for var in global_vars.iter().filter(|v| v.enabled) {
        map.insert(var.key.as_str(), var.value.as_str());
    }
    for var in collection_vars.iter().filter(|v| v.enabled) {
        map.insert(var.key.as_str(), var.value.as_str());
    }
    for var in env_vars.iter().filter(|v| v.enabled) {
        map.insert(var.key.as_str(), var.value.as_str());
    }

    map
}

/// Replace every `{{key}}` occurrence in `input` using `lookup`.
/// Unknown keys are left as-is (Requirement 3.5).
fn replace_placeholders(input: &str, lookup: &HashMap<&str, &str>) -> String {
    let mut result = String::with_capacity(input.len());
    let mut remaining = input;

    while let Some(start) = remaining.find("{{") {
        // Copy everything before the opening `{{`.
        result.push_str(&remaining[..start]);
        remaining = &remaining[start + 2..];

        // Look for the closing `}}`.
        if let Some(end) = remaining.find("}}") {
            let key = &remaining[..end];
            if let Some(&value) = lookup.get(key) {
                result.push_str(value);
            } else {
                // Key not found — leave placeholder unchanged.
                result.push_str("{{");
                result.push_str(key);
                result.push_str("}}");
            }
            remaining = &remaining[end + 2..];
        } else {
            // No closing `}}` found — restore the `{{` and stop scanning.
            result.push_str("{{");
            // remaining is already past the `{{` — append it as-is.
            result.push_str(remaining);
            return result;
        }
    }

    // Append any trailing text after the last `}}` (or the whole string if no
    // `{{` was found at all).
    result.push_str(remaining);
    result
}

/// Interpolate variable placeholders inside a `RequestBody`.
fn interpolate_body(body: RequestBody, lookup: &HashMap<&str, &str>) -> RequestBody {
    match body {
        RequestBody::Json { content } => RequestBody::Json {
            content: replace_placeholders(&content, lookup),
        },
        RequestBody::Raw {
            content,
            content_type,
        } => RequestBody::Raw {
            content: replace_placeholders(&content, lookup),
            content_type,
        },
        RequestBody::FormUrlEncoded { fields } => RequestBody::FormUrlEncoded {
            fields: interpolate_fields(fields, lookup),
        },
        RequestBody::Multipart { fields } => RequestBody::Multipart {
            fields: interpolate_fields(fields, lookup),
        },
        // None and Binary carry no interpolatable text fields.
        other => other,
    }
}

/// Interpolate values of enabled `MultipartField` entries.
fn interpolate_fields(
    mut fields: Vec<crate::models::MultipartField>,
    lookup: &HashMap<&str, &str>,
) -> Vec<crate::models::MultipartField> {
    for field in &mut fields {
        if field.enabled {
            field.value = replace_placeholders(&field.value, lookup);
        }
    }
    fields
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AuthConfig, Header, HttpMethod, MultipartField, QueryParam, RequestBody};

    // --- Helper builders ---

    fn make_var(key: &str, value: &str, enabled: bool) -> Variable {
        Variable {
            id: uuid::Uuid::new_v4().to_string(),
            environment_id: None,
            key: key.to_string(),
            value: value.to_string(),
            enabled,
        }
    }

    fn base_request(url: &str) -> Request {
        Request {
            id: uuid::Uuid::new_v4().to_string(),
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

    // --- replace_placeholders ---

    #[test]
    fn no_placeholders_returns_input_unchanged() {
        let lookup = HashMap::new();
        assert_eq!(
            replace_placeholders("https://example.com/api", &lookup),
            "https://example.com/api"
        );
    }

    #[test]
    fn known_placeholder_is_replaced() {
        let mut lookup = HashMap::new();
        lookup.insert("host", "api.example.com");
        assert_eq!(
            replace_placeholders("https://{{host}}/v1", &lookup),
            "https://api.example.com/v1"
        );
    }

    #[test]
    fn unknown_placeholder_is_left_unchanged() {
        let lookup = HashMap::new();
        assert_eq!(
            replace_placeholders("https://{{host}}/v1", &lookup),
            "https://{{host}}/v1"
        );
    }

    #[test]
    fn multiple_placeholders_replaced_independently() {
        let mut lookup = HashMap::new();
        lookup.insert("scheme", "https");
        lookup.insert("host", "example.com");
        // "path" is not in lookup — should remain.
        assert_eq!(
            replace_placeholders("{{scheme}}://{{host}}/{{path}}", &lookup),
            "https://example.com/{{path}}"
        );
    }

    #[test]
    fn unclosed_double_brace_is_left_unchanged() {
        let mut lookup = HashMap::new();
        lookup.insert("key", "value");
        // `{{key` has no closing `}}` — leave as-is.
        assert_eq!(replace_placeholders("{{key", &lookup), "{{key");
    }

    #[test]
    fn empty_placeholder_key_not_in_lookup_is_preserved() {
        let lookup = HashMap::new();
        assert_eq!(replace_placeholders("{{}}", &lookup), "{{}}");
    }

    // --- build_lookup priority ---

    #[test]
    fn env_var_overrides_collection_and_global() {
        let global = vec![make_var("BASE_URL", "https://global.example.com", true)];
        let collection = vec![make_var("BASE_URL", "https://collection.example.com", true)];
        let env = vec![make_var("BASE_URL", "https://env.example.com", true)];

        let lookup = build_lookup(&env, &collection, &global);
        assert_eq!(lookup["BASE_URL"], "https://env.example.com");
    }

    #[test]
    fn collection_var_overrides_global() {
        let global = vec![make_var("BASE_URL", "https://global.example.com", true)];
        let collection = vec![make_var("BASE_URL", "https://collection.example.com", true)];

        let lookup = build_lookup(&[], &collection, &global);
        assert_eq!(lookup["BASE_URL"], "https://collection.example.com");
    }

    #[test]
    fn disabled_variable_is_excluded_from_lookup() {
        let env = vec![make_var("TOKEN", "secret", false)];
        let lookup = build_lookup(&env, &[], &[]);
        assert!(!lookup.contains_key("TOKEN"));
    }

    // --- VarInterpolatorService::interpolate ---

    #[test]
    fn interpolate_url() {
        let env = vec![make_var("BASE_URL", "https://api.example.com", true)];
        let req = base_request("{{BASE_URL}}/todos");
        let out = VarInterpolatorService::interpolate(req, &env, &[], &[]);
        assert_eq!(out.url, "https://api.example.com/todos");
    }

    #[test]
    fn interpolate_enabled_header_value() {
        let env = vec![make_var("TOKEN", "tok_abc", true)];
        let mut req = base_request("https://example.com");
        req.headers = vec![Header {
            key: "Authorization".to_string(),
            value: "Bearer {{TOKEN}}".to_string(),
            enabled: true,
        }];
        let out = VarInterpolatorService::interpolate(req, &env, &[], &[]);
        assert_eq!(out.headers[0].value, "Bearer tok_abc");
    }

    #[test]
    fn disabled_header_value_not_interpolated() {
        let env = vec![make_var("TOKEN", "tok_abc", true)];
        let mut req = base_request("https://example.com");
        req.headers = vec![Header {
            key: "Authorization".to_string(),
            value: "Bearer {{TOKEN}}".to_string(),
            enabled: false,
        }];
        let out = VarInterpolatorService::interpolate(req, &env, &[], &[]);
        // Disabled header value should remain unchanged.
        assert_eq!(out.headers[0].value, "Bearer {{TOKEN}}");
    }

    #[test]
    fn interpolate_enabled_query_param_value() {
        let env = vec![make_var("API_KEY", "12345", true)];
        let mut req = base_request("https://example.com");
        req.params = vec![QueryParam {
            key: "apikey".to_string(),
            value: "{{API_KEY}}".to_string(),
            enabled: true,
        }];
        let out = VarInterpolatorService::interpolate(req, &env, &[], &[]);
        assert_eq!(out.params[0].value, "12345");
    }

    #[test]
    fn disabled_param_value_not_interpolated() {
        let env = vec![make_var("API_KEY", "12345", true)];
        let mut req = base_request("https://example.com");
        req.params = vec![QueryParam {
            key: "apikey".to_string(),
            value: "{{API_KEY}}".to_string(),
            enabled: false,
        }];
        let out = VarInterpolatorService::interpolate(req, &env, &[], &[]);
        assert_eq!(out.params[0].value, "{{API_KEY}}");
    }

    #[test]
    fn interpolate_json_body() {
        let env = vec![make_var("USER_ID", "42", true)];
        let mut req = base_request("https://example.com");
        req.body = RequestBody::Json {
            content: r#"{"id":"{{USER_ID}}"}"#.to_string(),
        };
        let out = VarInterpolatorService::interpolate(req, &env, &[], &[]);
        assert_eq!(
            out.body,
            RequestBody::Json {
                content: r#"{"id":"42"}"#.to_string()
            }
        );
    }

    #[test]
    fn interpolate_raw_body() {
        let env = vec![make_var("NAME", "Alice", true)];
        let mut req = base_request("https://example.com");
        req.body = RequestBody::Raw {
            content: "Hello {{NAME}}!".to_string(),
            content_type: "text/plain".to_string(),
        };
        let out = VarInterpolatorService::interpolate(req, &env, &[], &[]);
        assert_eq!(
            out.body,
            RequestBody::Raw {
                content: "Hello Alice!".to_string(),
                content_type: "text/plain".to_string(),
            }
        );
    }

    #[test]
    fn interpolate_form_url_encoded_fields() {
        let env = vec![make_var("USERNAME", "bob", true)];
        let mut req = base_request("https://example.com");
        req.body = RequestBody::FormUrlEncoded {
            fields: vec![
                MultipartField {
                    key: "user".to_string(),
                    value: "{{USERNAME}}".to_string(),
                    enabled: true,
                },
                MultipartField {
                    key: "disabled_field".to_string(),
                    value: "{{USERNAME}}".to_string(),
                    enabled: false,
                },
            ],
        };
        let out = VarInterpolatorService::interpolate(req, &env, &[], &[]);
        if let RequestBody::FormUrlEncoded { fields } = out.body {
            assert_eq!(fields[0].value, "bob");
            assert_eq!(fields[1].value, "{{USERNAME}}"); // disabled — unchanged
        } else {
            panic!("Expected FormUrlEncoded body");
        }
    }

    #[test]
    fn interpolate_multipart_fields() {
        let env = vec![make_var("DESC", "hello world", true)];
        let mut req = base_request("https://example.com");
        req.body = RequestBody::Multipart {
            fields: vec![MultipartField {
                key: "description".to_string(),
                value: "{{DESC}}".to_string(),
                enabled: true,
            }],
        };
        let out = VarInterpolatorService::interpolate(req, &env, &[], &[]);
        if let RequestBody::Multipart { fields } = out.body {
            assert_eq!(fields[0].value, "hello world");
        } else {
            panic!("Expected Multipart body");
        }
    }

    #[test]
    fn binary_body_not_modified() {
        let env = vec![make_var("PATH", "/tmp/file", true)];
        let mut req = base_request("https://example.com");
        req.body = RequestBody::Binary {
            file_path: "{{PATH}}".to_string(),
        };
        let out = VarInterpolatorService::interpolate(req, &env, &[], &[]);
        // Binary bodies are not interpolated.
        assert_eq!(
            out.body,
            RequestBody::Binary {
                file_path: "{{PATH}}".to_string()
            }
        );
    }

    #[test]
    fn unresolved_placeholder_remains_in_url() {
        let req = base_request("https://{{MISSING_HOST}}/api");
        let out = VarInterpolatorService::interpolate(req, &[], &[], &[]);
        assert_eq!(out.url, "https://{{MISSING_HOST}}/api");
    }
}
