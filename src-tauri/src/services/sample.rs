// Sample service — returns a static list of built-in Request presets.
//
// Task 12.1: Return static list of Request presets with deterministic UUIDs.
// Requirements: 10.1, 10.3

use crate::models::{AuthConfig, Header, HttpMethod, QueryParam, Request, RequestBody};

/// Returns the four built-in sample requests.
///
/// Each sample has a deterministic UUID (v5-style, but hardcoded), a valid URL,
/// and a valid `HttpMethod`.  They are safe to display immediately without any
/// storage round-trip.
pub fn list_samples() -> Vec<Request> {
    let now = "2024-01-01T00:00:00Z".to_string();

    vec![
        // ------------------------------------------------------------------
        // 1. GET JSONPlaceholder /todos/1
        // ------------------------------------------------------------------
        Request {
            id: "00000000-0000-0000-0000-000000000001".to_string(),
            collection_id: "00000000-0000-0000-0000-000000000000".to_string(),
            name: "GET JSONPlaceholder Todo".to_string(),
            method: HttpMethod::Get,
            url: "https://jsonplaceholder.typicode.com/todos/1".to_string(),
            headers: vec![Header {
                key: "Accept".to_string(),
                value: "application/json".to_string(),
                enabled: true,
            }],
            params: vec![],
            body: RequestBody::None,
            auth: AuthConfig::None,
            position: 0,
            created_at: now.clone(),
            updated_at: now.clone(),
        },
        // ------------------------------------------------------------------
        // 2. POST JSONPlaceholder /todos
        // ------------------------------------------------------------------
        Request {
            id: "00000000-0000-0000-0000-000000000002".to_string(),
            collection_id: "00000000-0000-0000-0000-000000000000".to_string(),
            name: "POST JSONPlaceholder Todo".to_string(),
            method: HttpMethod::Post,
            url: "https://jsonplaceholder.typicode.com/todos".to_string(),
            headers: vec![
                Header {
                    key: "Content-Type".to_string(),
                    value: "application/json".to_string(),
                    enabled: true,
                },
                Header {
                    key: "Accept".to_string(),
                    value: "application/json".to_string(),
                    enabled: true,
                },
            ],
            params: vec![],
            body: RequestBody::Json {
                content: "{\n  \"title\": \"Sample todo\",\n  \"completed\": false,\n  \"userId\": 1\n}"
                    .to_string(),
            },
            auth: AuthConfig::None,
            position: 1,
            created_at: now.clone(),
            updated_at: now.clone(),
        },
        // ------------------------------------------------------------------
        // 3. GET httpbin.org /get
        // ------------------------------------------------------------------
        Request {
            id: "00000000-0000-0000-0000-000000000003".to_string(),
            collection_id: "00000000-0000-0000-0000-000000000000".to_string(),
            name: "GET httpbin".to_string(),
            method: HttpMethod::Get,
            url: "https://httpbin.org/get".to_string(),
            headers: vec![Header {
                key: "Accept".to_string(),
                value: "application/json".to_string(),
                enabled: true,
            }],
            params: vec![QueryParam {
                key: "sample".to_string(),
                value: "lightman".to_string(),
                enabled: true,
            }],
            body: RequestBody::None,
            auth: AuthConfig::None,
            position: 2,
            created_at: now.clone(),
            updated_at: now.clone(),
        },
        // ------------------------------------------------------------------
        // 4. POST httpbin.org /post
        // ------------------------------------------------------------------
        Request {
            id: "00000000-0000-0000-0000-000000000004".to_string(),
            collection_id: "00000000-0000-0000-0000-000000000000".to_string(),
            name: "POST httpbin".to_string(),
            method: HttpMethod::Post,
            url: "https://httpbin.org/post".to_string(),
            headers: vec![
                Header {
                    key: "Content-Type".to_string(),
                    value: "application/json".to_string(),
                    enabled: true,
                },
                Header {
                    key: "Accept".to_string(),
                    value: "application/json".to_string(),
                    enabled: true,
                },
            ],
            params: vec![],
            body: RequestBody::Json {
                content: "{\n  \"message\": \"Hello from Lightman\"\n}".to_string(),
            },
            auth: AuthConfig::None,
            position: 3,
            created_at: now.clone(),
            updated_at: now.clone(),
        },
    ]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_samples_returns_four_presets() {
        let samples = list_samples();
        assert_eq!(samples.len(), 4);
    }

    #[test]
    fn all_samples_have_unique_deterministic_ids() {
        let samples = list_samples();
        let ids: Vec<&str> = samples.iter().map(|r| r.id.as_str()).collect();

        // All IDs are non-empty.
        assert!(ids.iter().all(|id| !id.is_empty()));

        // All IDs are unique.
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "duplicate sample IDs found");

        // IDs are deterministic — calling twice returns the same values.
        let samples2 = list_samples();
        let ids2: Vec<&str> = samples2.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ids2);
    }

    #[test]
    fn all_samples_have_valid_urls() {
        let samples = list_samples();
        for sample in &samples {
            assert!(
                sample.url.starts_with("https://"),
                "sample '{}' has invalid URL: {}",
                sample.name,
                sample.url
            );
        }
    }

    #[test]
    fn all_samples_have_non_empty_names() {
        let samples = list_samples();
        for sample in &samples {
            assert!(!sample.name.is_empty(), "sample has empty name");
        }
    }

    #[test]
    fn first_sample_is_get_jsonplaceholder_todos() {
        let samples = list_samples();
        let s = &samples[0];
        assert_eq!(s.method, HttpMethod::Get);
        assert!(
            s.url.contains("jsonplaceholder.typicode.com/todos/1"),
            "expected /todos/1 URL, got: {}",
            s.url
        );
    }

    #[test]
    fn second_sample_is_post_jsonplaceholder_todos() {
        let samples = list_samples();
        let s = &samples[1];
        assert_eq!(s.method, HttpMethod::Post);
        assert!(
            s.url.contains("jsonplaceholder.typicode.com/todos"),
            "expected /todos URL, got: {}",
            s.url
        );
        // Body should be JSON.
        assert!(
            matches!(s.body, RequestBody::Json { .. }),
            "expected JSON body for POST sample"
        );
    }

    #[test]
    fn third_sample_is_get_httpbin() {
        let samples = list_samples();
        let s = &samples[2];
        assert_eq!(s.method, HttpMethod::Get);
        assert!(
            s.url.contains("httpbin.org/get"),
            "expected /get URL, got: {}",
            s.url
        );
    }

    #[test]
    fn fourth_sample_is_post_httpbin() {
        let samples = list_samples();
        let s = &samples[3];
        assert_eq!(s.method, HttpMethod::Post);
        assert!(
            s.url.contains("httpbin.org/post"),
            "expected /post URL, got: {}",
            s.url
        );
        assert!(
            matches!(s.body, RequestBody::Json { .. }),
            "expected JSON body for POST httpbin sample"
        );
    }

    #[test]
    fn positions_are_sequential() {
        let samples = list_samples();
        for (i, sample) in samples.iter().enumerate() {
            assert_eq!(
                sample.position, i as i64,
                "expected position {i} for sample '{}'",
                sample.name
            );
        }
    }
}
