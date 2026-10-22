// Property-based tests untuk sample request integrity (Property 17).
//
// Task 12.3 — Property 17: Sample Requests Are Well-Formed
//   Setiap request yang dikembalikan oleh `list_samples` harus memenuhi
//   invariant berikut tanpa terkecuali:
//
//   17a. Non-empty ID        — `id` tidak boleh kosong.
//   17b. UUID-like ID        — `id` harus memiliki format UUID (8-4-4-4-12).
//   17c. Non-empty name      — `name` tidak boleh kosong.
//   17d. Name length ≤ 255   — `name` tidak boleh melebihi 255 karakter.
//   17e. Valid URL prefix    — `url` harus dimulai dengan "https://" atau "http://".
//   17f. Non-empty URL       — `url` tidak boleh kosong.
//   17g. Valid method        — `method` harus salah satu dari HttpMethod yang didukung.
//   17h. Unique IDs          — tidak ada dua sample yang memiliki `id` yang sama.
//   17i. Sequential position — `position` harus berurutan mulai dari 0.
//   17j. Deterministic       — dua panggilan `list_samples()` menghasilkan hasil identik.
//   17k. Non-empty headers   — setiap header memiliki `key` dan `value` non-empty.
//   17l. Non-empty params    — setiap query param memiliki `key` dan `value` non-empty.
//   17m. JSON body validity  — body `RequestBody::Json` harus berisi valid JSON.
//
// Validates: Requirements 10.1, 10.3
// File: tests/property/prop_samples.rs

use proptest::prelude::*;
use lightman_lib::models::{HttpMethod, RequestBody};
use lightman_lib::services::sample::list_samples;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Regexp untuk memverifikasi format UUID standar: 8-4-4-4-12 hex chars.
fn is_uuid_format(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 5 {
        return false;
    }
    let expected_lens = [8usize, 4, 4, 4, 12];
    parts
        .iter()
        .zip(expected_lens.iter())
        .all(|(part, &len)| part.len() == len && part.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Kembalikan `true` jika `url` dimulai dengan skema HTTP yang valid.
fn has_valid_url_scheme(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

/// Kembalikan `true` jika string adalah JSON valid.
fn is_valid_json(s: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(s).is_ok()
}

// ---------------------------------------------------------------------------
// Unit sanity checks — run once, no proptest overhead
// ---------------------------------------------------------------------------

#[test]
fn samples_count_is_exactly_four() {
    let samples = list_samples();
    assert_eq!(
        samples.len(),
        4,
        "list_samples must return exactly 4 presets"
    );
}

#[test]
fn all_sample_ids_are_non_empty() {
    for s in list_samples() {
        assert!(!s.id.is_empty(), "sample '{}' has empty id", s.name);
    }
}

#[test]
fn all_sample_ids_have_uuid_format() {
    for s in list_samples() {
        assert!(
            is_uuid_format(&s.id),
            "sample '{}' has non-UUID id: {}",
            s.name,
            s.id
        );
    }
}

#[test]
fn all_sample_names_are_non_empty() {
    for s in list_samples() {
        assert!(!s.name.is_empty(), "a sample has empty name");
    }
}

#[test]
fn all_sample_names_are_within_255_chars() {
    for s in list_samples() {
        assert!(
            s.name.chars().count() <= 255,
            "sample name '{}' exceeds 255 characters",
            s.name
        );
    }
}

#[test]
fn all_sample_urls_have_valid_scheme() {
    for s in list_samples() {
        assert!(
            has_valid_url_scheme(&s.url),
            "sample '{}' has invalid URL scheme: {}",
            s.name,
            s.url
        );
    }
}

#[test]
fn all_sample_urls_are_non_empty() {
    for s in list_samples() {
        assert!(!s.url.is_empty(), "sample '{}' has empty URL", s.name);
    }
}

#[test]
fn all_sample_ids_are_unique() {
    let samples = list_samples();
    let mut ids: Vec<&str> = samples.iter().map(|s| s.id.as_str()).collect();
    let original_len = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(
        ids.len(),
        original_len,
        "duplicate IDs found among sample requests"
    );
}

#[test]
fn sample_positions_are_sequential_from_zero() {
    for (i, s) in list_samples().iter().enumerate() {
        assert_eq!(
            s.position,
            i as i64,
            "sample '{}' has position {} but expected {}",
            s.name,
            s.position,
            i
        );
    }
}

#[test]
fn list_samples_is_deterministic() {
    let first = list_samples();
    let second = list_samples();
    assert_eq!(
        first.len(),
        second.len(),
        "list_samples returned different counts on two calls"
    );
    for (a, b) in first.iter().zip(second.iter()) {
        assert_eq!(a.id, b.id, "id changed between calls");
        assert_eq!(a.name, b.name, "name changed between calls");
        assert_eq!(a.url, b.url, "url changed between calls");
        assert_eq!(a.method, b.method, "method changed between calls");
        assert_eq!(a.position, b.position, "position changed between calls");
    }
}

#[test]
fn header_keys_and_values_are_non_empty() {
    for s in list_samples() {
        for h in &s.headers {
            assert!(
                !h.key.is_empty(),
                "sample '{}' has a header with empty key",
                s.name
            );
            assert!(
                !h.value.is_empty(),
                "sample '{}' has header '{}' with empty value",
                s.name,
                h.key
            );
        }
    }
}

#[test]
fn query_param_keys_and_values_are_non_empty() {
    for s in list_samples() {
        for p in &s.params {
            assert!(
                !p.key.is_empty(),
                "sample '{}' has a query param with empty key",
                s.name
            );
            assert!(
                !p.value.is_empty(),
                "sample '{}' has query param '{}' with empty value",
                s.name,
                p.key
            );
        }
    }
}

#[test]
fn json_bodies_contain_valid_json() {
    for s in list_samples() {
        if let RequestBody::Json { content } = &s.body {
            assert!(
                is_valid_json(content),
                "sample '{}' has invalid JSON body: {}",
                s.name,
                content
            );
        }
    }
}

#[test]
fn post_samples_have_json_body() {
    for s in list_samples() {
        if s.method == HttpMethod::Post {
            assert!(
                matches!(s.body, RequestBody::Json { .. }),
                "POST sample '{}' should have a JSON body",
                s.name
            );
        }
    }
}

#[test]
fn get_samples_have_no_body() {
    for s in list_samples() {
        if s.method == HttpMethod::Get {
            assert!(
                matches!(s.body, RequestBody::None),
                "GET sample '{}' should have no body",
                s.name
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Property 17 — proptest-based: Sample Requests Are Well-Formed
//
// Proptest di sini memverifikasi bahwa setiap elemen dari list_samples()
// memenuhi semua invariant di atas, termasuk dalam konteks indeks arbitrer.
// Karena `list_samples()` adalah pure function tanpa parameter, strategi yang
// digunakan adalah index arbitrary untuk memilih sample yang akan diperiksa.
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// 17a+17b: Setiap sample yang dipilih secara arbitrer memiliki id non-empty
    /// dengan format UUID yang valid.
    #[test]
    fn prop_sample_id_is_non_empty_uuid(index in 0usize..4usize) {
        let samples = list_samples();
        let s = &samples[index];

        prop_assert!(
            !s.id.is_empty(),
            "sample[{index}] '{}' has empty id",
            s.name
        );
        prop_assert!(
            is_uuid_format(&s.id),
            "sample[{index}] '{}' id '{}' is not UUID format",
            s.name,
            s.id
        );
    }

    /// 17c+17d: Setiap sample memiliki nama yang valid (1–255 karakter).
    #[test]
    fn prop_sample_name_is_valid(index in 0usize..4usize) {
        let samples = list_samples();
        let s = &samples[index];
        let char_count = s.name.chars().count();

        prop_assert!(
            char_count >= 1,
            "sample[{index}] has empty name"
        );
        prop_assert!(
            char_count <= 255,
            "sample[{index}] name '{}' exceeds 255 chars (count={})",
            s.name,
            char_count
        );
    }

    /// 17e+17f: URL setiap sample tidak boleh kosong dan harus dimulai dengan
    /// skema https:// atau http://.
    #[test]
    fn prop_sample_url_is_well_formed(index in 0usize..4usize) {
        let samples = list_samples();
        let s = &samples[index];

        prop_assert!(
            !s.url.is_empty(),
            "sample[{index}] '{}' has empty URL",
            s.name
        );
        prop_assert!(
            has_valid_url_scheme(&s.url),
            "sample[{index}] '{}' url '{}' has no valid scheme",
            s.name,
            s.url
        );
    }

    /// 17h: Tidak ada dua sample (dengan indeks berbeda) yang memiliki ID sama.
    #[test]
    fn prop_sample_ids_are_pairwise_unique(
        i in 0usize..4usize,
        j in 0usize..4usize,
    ) {
        prop_assume!(i != j);

        let samples = list_samples();
        prop_assert_ne!(
            &samples[i].id,
            &samples[j].id,
            "samples[{}] and samples[{}] share the same id: {}",
            i,
            j,
            samples[i].id
        );
    }

    /// 17i: Position setiap sample harus sama dengan indeksnya dalam list.
    #[test]
    fn prop_sample_position_equals_index(index in 0usize..4usize) {
        let samples = list_samples();
        let s = &samples[index];

        prop_assert_eq!(
            s.position,
            index as i64,
            "sample '{}' at index {} has position {}",
            s.name,
            index,
            s.position
        );
    }

    /// 17j: Dua panggilan list_samples() menghasilkan elemen yang identik
    /// untuk indeks yang sama.
    #[test]
    fn prop_list_samples_is_deterministic(index in 0usize..4usize) {
        let first  = list_samples();
        let second = list_samples();

        prop_assert_eq!(&first[index].id,       &second[index].id,       "id not stable");
        prop_assert_eq!(&first[index].name,     &second[index].name,     "name not stable");
        prop_assert_eq!(&first[index].url,      &second[index].url,      "url not stable");
        prop_assert_eq!(&first[index].method,   &second[index].method,   "method not stable");
        prop_assert_eq!( first[index].position,  second[index].position,  "position not stable");
    }

    /// 17k: Setiap header dalam sample yang dipilih memiliki key dan value non-empty.
    #[test]
    fn prop_sample_headers_are_non_empty(index in 0usize..4usize) {
        let samples = list_samples();
        let s = &samples[index];

        for h in &s.headers {
            prop_assert!(
                !h.key.is_empty(),
                "sample '{}' has header with empty key",
                s.name
            );
            prop_assert!(
                !h.value.is_empty(),
                "sample '{}' has header '{}' with empty value",
                s.name,
                h.key
            );
        }
    }

    /// 17l: Setiap query param dalam sample yang dipilih memiliki key dan value non-empty.
    #[test]
    fn prop_sample_params_are_non_empty(index in 0usize..4usize) {
        let samples = list_samples();
        let s = &samples[index];

        for p in &s.params {
            prop_assert!(
                !p.key.is_empty(),
                "sample '{}' has query param with empty key",
                s.name
            );
            prop_assert!(
                !p.value.is_empty(),
                "sample '{}' has query param '{}' with empty value",
                s.name,
                p.key
            );
        }
    }

    /// 17m: Body RequestBody::Json dalam sample harus mengandung JSON yang valid.
    #[test]
    fn prop_sample_json_body_is_valid_json(index in 0usize..4usize) {
        let samples = list_samples();
        let s = &samples[index];

        if let RequestBody::Json { content } = &s.body {
            prop_assert!(
                is_valid_json(content),
                "sample '{}' JSON body is not valid JSON: {}",
                s.name,
                content
            );
        }
    }

    /// Konsistensi method-body: POST harus punya JSON body, GET harus None.
    #[test]
    fn prop_sample_method_body_consistency(index in 0usize..4usize) {
        let samples = list_samples();
        let s = &samples[index];

        match s.method {
            HttpMethod::Get => {
                prop_assert!(
                    matches!(s.body, RequestBody::None),
                    "GET sample '{}' should have RequestBody::None, got something else",
                    s.name
                );
            }
            HttpMethod::Post => {
                prop_assert!(
                    matches!(s.body, RequestBody::Json { .. }),
                    "POST sample '{}' should have RequestBody::Json",
                    s.name
                );
            }
            _ => {
                // Metode lain tidak ada di samples saat ini — tidak ada assertion.
            }
        }
    }
}
