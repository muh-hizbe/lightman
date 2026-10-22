// Property-based tests untuk header merge behavior (Properties 15, 16).
//
// Task 6.3 — dua property divalidasi:
//
//   Property 15: Auth Header Overrides Manual Header
//     Untuk sembarang auth config yang menginjeksi header (BasicAuth, BearerToken,
//     ApiKey-as-header), jika request sudah mempunyai header manual dengan key yang
//     sama, hasil merge harus menggunakan nilai dari auth, bukan dari header manual.
//     Validates: Requirement 9.6
//
//   Property 16: Merge Preserves Non-Conflicting Headers
//     Headers yang tidak berkonflik (key berbeda antara collection default dan
//     request level) harus semuanya ada dalam output merge. Tidak ada header yang
//     hilang secara diam-diam.
//     Validates: Requirement 9.6

use proptest::prelude::*;

use lightman_lib::models::{ApiKeyLocation, AuthConfig, Header, HttpMethod, Request, RequestBody};
use lightman_lib::services::header_merger::HeaderMergerService;

// ---------------------------------------------------------------------------
// Strategies
// ---------------------------------------------------------------------------

/// Arbitrary header key yang valid: huruf, digit, dan tanda hubung.
/// Dimulai dengan huruf agar tidak ada edge-case parsing.
fn arbitrary_header_key() -> impl Strategy<Value = String> {
    proptest::string::string_regex("[A-Za-z][A-Za-z0-9\\-]{0,29}")
        .expect("arbitrary_header_key regex should compile")
}

/// Arbitrary header value — printable ASCII, panjang 1–64 karakter.
fn arbitrary_header_value() -> impl Strategy<Value = String> {
    proptest::string::string_regex("[A-Za-z0-9 !@#$%^&*()_+\\-=.,:;?/]{1,64}")
        .expect("arbitrary_header_value regex should compile")
}

/// Arbitrary credential string untuk BasicAuth username/password/token.
fn arbitrary_credential() -> impl Strategy<Value = String> {
    proptest::string::string_regex("[A-Za-z0-9 !@#$%^&*()_+\\-=\\[\\]{}|;:',.<>?/~`]{0,64}")
        .expect("arbitrary_credential regex should compile")
}

/// Arbitrary token string untuk BearerToken.
fn arbitrary_token() -> impl Strategy<Value = String> {
    proptest::string::string_regex("[A-Za-z0-9._\\-+/=]{1,128}")
        .expect("arbitrary_token regex should compile")
}

/// Arbitrary ApiKey key name (header name atau query param name).
fn arbitrary_api_key_name() -> impl Strategy<Value = String> {
    proptest::string::string_regex("[A-Za-z][A-Za-z0-9\\-]{0,29}")
        .expect("arbitrary_api_key_name regex should compile")
}

/// Sebuah vector 0–5 header yang diaktifkan (enabled = true), dengan key unik.
/// Digunakan untuk mensimulasikan default headers koleksi atau request headers.
fn arbitrary_enabled_headers(
    max_count: usize,
) -> impl Strategy<Value = Vec<Header>> {
    proptest::collection::vec(
        (arbitrary_header_key(), arbitrary_header_value()),
        0..=max_count,
    )
    .prop_map(|pairs| {
        // Deduplikasi berdasarkan key (case-insensitive) agar tidak ada duplikat
        // dalam satu sumber headers — ini mensimulasikan input yang wajar.
        let mut seen: Vec<String> = Vec::new();
        let mut headers: Vec<Header> = Vec::new();
        for (key, value) in pairs {
            if !seen.iter().any(|s: &String| s.eq_ignore_ascii_case(&key)) {
                seen.push(key.clone());
                headers.push(Header {
                    key,
                    value,
                    enabled: true,
                });
            }
        }
        headers
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Buat `Request` minimal dengan auth dan request-level headers yang diberikan.
fn request_with(auth: AuthConfig, headers: Vec<Header>) -> Request {
    Request {
        id: "req-prop".to_string(),
        collection_id: "col-prop".to_string(),
        name: "prop-test".to_string(),
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

/// Hitung jumlah header dalam `headers` yang keynya cocok (case-insensitive).
fn count_headers_by_key(headers: &[Header], key: &str) -> usize {
    headers
        .iter()
        .filter(|h| h.key.eq_ignore_ascii_case(key))
        .count()
}

/// Ambil nilai header pertama yang keynya cocok (case-insensitive).
fn find_header_value<'a>(headers: &'a [Header], key: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|h| h.key.eq_ignore_ascii_case(key))
        .map(|h| h.value.as_str())
}

// ---------------------------------------------------------------------------
// Property 15 — Auth Header Overrides Manual Header
// ---------------------------------------------------------------------------
//
// Sifat yang divalidasi (berlaku untuk ketiga auth scheme yang menginjeksi header):
//   1. Hanya ada tepat satu header dengan key yang diklaim auth dalam output.
//   2. Nilainya adalah nilai yang dihasilkan auth, bukan nilai manual.
//   3. Semua header *lain* yang tidak berkonflik dengan key auth tetap ada.

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    // -----------------------------------------------------------------------
    // Property 15a — BasicAuth menimpa header Authorization manual
    // -----------------------------------------------------------------------

    /// Jika request mempunyai header `Authorization` manual dengan nilai
    /// sembarang, BasicAuth harus menimpanya.  Output harus mempunyai tepat
    /// satu `Authorization` dengan format `"Basic <base64>"`.
    #[test]
    fn prop_basic_auth_overrides_any_manual_authorization_header(
        username     in arbitrary_credential(),
        password     in arbitrary_credential(),
        manual_value in arbitrary_header_value(),
        extra_headers in arbitrary_enabled_headers(4),
    ) {
        // Pastikan tidak ada header `Authorization` dalam extra_headers agar
        // tidak mengganggu counting assertion.
        prop_assume!(
            !extra_headers
                .iter()
                .any(|h| h.key.eq_ignore_ascii_case("Authorization"))
        );

        let mut req_headers = extra_headers.clone();
        req_headers.push(Header {
            key:     "Authorization".to_string(),
            value:   manual_value.clone(),
            enabled: true,
        });

        let auth = AuthConfig::BasicAuth {
            username: username.clone(),
            password: password.clone(),
        };
        let req    = request_with(auth, req_headers);
        let result = HeaderMergerService::merge(&req, &[], &[]);

        // Tepat satu Authorization header.
        let count = count_headers_by_key(&result.headers, "Authorization");
        prop_assert_eq!(
            count,
            1,
            "must have exactly one Authorization header, got {}",
            count
        );

        // Nilainya harus dimulai dengan "Basic ".
        let value = find_header_value(&result.headers, "Authorization").unwrap();
        prop_assert!(
            value.starts_with("Basic "),
            "Authorization must start with 'Basic ', got: '{}'",
            value
        );

        // Header lain yang tidak berkonflik tetap ada.
        for extra in &extra_headers {
            let found = count_headers_by_key(&result.headers, &extra.key);
            prop_assert_eq!(
                found,
                1,
                "non-conflicting header '{}' must be preserved",
                extra.key
            );
        }
    }

    // -----------------------------------------------------------------------
    // Property 15b — BearerToken menimpa header Authorization manual
    // -----------------------------------------------------------------------

    /// Jika request mempunyai header `Authorization` manual, BearerToken harus
    /// menimpanya. Output harus mempunyai tepat satu `Authorization` dengan
    /// format `"Bearer <token>"`.
    #[test]
    fn prop_bearer_token_overrides_any_manual_authorization_header(
        token        in arbitrary_token(),
        manual_value in arbitrary_header_value(),
        extra_headers in arbitrary_enabled_headers(4),
    ) {
        prop_assume!(
            !extra_headers
                .iter()
                .any(|h| h.key.eq_ignore_ascii_case("Authorization"))
        );

        let mut req_headers = extra_headers.clone();
        req_headers.push(Header {
            key:     "Authorization".to_string(),
            value:   manual_value.clone(),
            enabled: true,
        });

        let auth = AuthConfig::BearerToken { token: token.clone() };
        let req    = request_with(auth, req_headers);
        let result = HeaderMergerService::merge(&req, &[], &[]);

        // Tepat satu Authorization header.
        let count = count_headers_by_key(&result.headers, "Authorization");
        prop_assert_eq!(
            count,
            1,
            "must have exactly one Authorization header, got {}",
            count
        );

        // Nilainya harus sama dengan "Bearer <token>".
        let value = find_header_value(&result.headers, "Authorization").unwrap();
        let expected = format!("Bearer {token}");
        prop_assert_eq!(
            value,
            expected.as_str(),
            "Authorization must equal 'Bearer <token>'"
        );

        // Header lain tetap ada.
        for extra in &extra_headers {
            let found = count_headers_by_key(&result.headers, &extra.key);
            prop_assert_eq!(
                found,
                1,
                "non-conflicting header '{}' must be preserved",
                extra.key
            );
        }
    }

    // -----------------------------------------------------------------------
    // Property 15c — ApiKey (as Header) menimpa header manual dengan key sama
    // -----------------------------------------------------------------------

    /// Jika request mempunyai header manual dengan key yang sama dengan ApiKey,
    /// nilai ApiKey dari auth harus menang.  Output harus mempunyai tepat satu
    /// header dengan key tersebut, bernilai dari auth.
    #[test]
    fn prop_api_key_header_overrides_manual_header_with_same_key(
        api_key_name  in arbitrary_api_key_name(),
        api_key_value in arbitrary_header_value(),
        manual_value  in arbitrary_header_value(),
        extra_headers in arbitrary_enabled_headers(3),
    ) {
        // Hindari konflik antara extra_headers dan api_key_name.
        prop_assume!(
            !extra_headers
                .iter()
                .any(|h| h.key.eq_ignore_ascii_case(&api_key_name))
        );
        // Manual value berbeda dari auth value agar assertion bermakna.
        prop_assume!(manual_value != api_key_value);

        let mut req_headers = extra_headers.clone();
        req_headers.push(Header {
            key:     api_key_name.clone(),
            value:   manual_value.clone(),
            enabled: true,
        });

        let auth = AuthConfig::ApiKey {
            key:      api_key_name.clone(),
            value:    api_key_value.clone(),
            location: ApiKeyLocation::Header,
        };
        let req    = request_with(auth, req_headers);
        let result = HeaderMergerService::merge(&req, &[], &[]);

        // Tepat satu header dengan api_key_name.
        let count = count_headers_by_key(&result.headers, &api_key_name);
        prop_assert_eq!(
            count,
            1,
            "must have exactly one '{}' header, got {}",
            api_key_name,
            count
        );

        // Nilainya harus dari auth, bukan dari manual.
        let value = find_header_value(&result.headers, &api_key_name).unwrap();
        prop_assert_eq!(
            value,
            api_key_value.as_str(),
            "api key header value must come from auth, not manual header"
        );

        // Header lain tidak berkonflik tetap ada.
        for extra in &extra_headers {
            let found = count_headers_by_key(&result.headers, &extra.key);
            prop_assert_eq!(
                found,
                1,
                "non-conflicting header '{}' must be preserved",
                extra.key
            );
        }
    }

    // -----------------------------------------------------------------------
    // Property 15d — Auth menimpa collection default header yang sama namanya
    // -----------------------------------------------------------------------

    /// BearerToken juga harus menimpa collection default `Authorization` header
    /// (bukan hanya request-level header manual).
    #[test]
    fn prop_auth_overrides_collection_default_authorization_header(
        token              in arbitrary_token(),
        collection_value   in arbitrary_header_value(),
    ) {
        let collection_defaults = vec![Header {
            key:     "Authorization".to_string(),
            value:   collection_value.clone(),
            enabled: true,
        }];

        let auth = AuthConfig::BearerToken { token: token.clone() };
        let req  = request_with(auth, vec![]);
        let result = HeaderMergerService::merge(&req, &collection_defaults, &[]);

        // Tepat satu Authorization header.
        let count = count_headers_by_key(&result.headers, "Authorization");
        prop_assert_eq!(
            count,
            1,
            "must have exactly one Authorization header"
        );

        // Nilainya dari auth, bukan dari collection default.
        let value = find_header_value(&result.headers, "Authorization").unwrap();
        let expected = format!("Bearer {token}");
        prop_assert_eq!(
            value,
            expected.as_str(),
            "auth must override collection default Authorization header"
        );
    }
}

// ---------------------------------------------------------------------------
// Property 16 — Merge Preserves Non-Conflicting Headers
// ---------------------------------------------------------------------------
//
// Sifat yang divalidasi:
//   1. Setiap header yang hanya ada di collection default (tidak berkonflik
//      dengan request headers atau auth) tetap ada dalam output.
//   2. Setiap header yang hanya ada di request (tidak berkonflik dengan
//      collection default atau auth) tetap ada dalam output.
//   3. Jumlah total headers dalam output adalah jumlah header unik dari
//      gabungan collection default dan request, plus header yang diinjeksi auth
//      (yang tidak berkonflik dengan keduanya).

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    // -----------------------------------------------------------------------
    // Property 16a — Semua non-conflicting headers dipertahankan (AuthConfig::None)
    // -----------------------------------------------------------------------

    /// Ketika tidak ada auth, output harus mengandung tepat satu entri untuk
    /// setiap key unik dari collection default dan request headers (case-insensitive).
    /// Request-level value menang jika ada konflik key.
    #[test]
    fn prop_merge_preserves_all_non_conflicting_headers_no_auth(
        col_headers in arbitrary_enabled_headers(4),
        req_headers in arbitrary_enabled_headers(4),
    ) {
        // Pisahkan: key yang hanya ada di col (tidak ada di req).
        let col_only_keys: Vec<String> = col_headers
            .iter()
            .filter(|ch| {
                !req_headers
                    .iter()
                    .any(|rh| rh.key.eq_ignore_ascii_case(&ch.key))
            })
            .map(|h| h.key.clone())
            .collect();

        // Key yang hanya ada di req (tidak ada di col).
        let req_only_keys: Vec<String> = req_headers
            .iter()
            .filter(|rh| {
                !col_headers
                    .iter()
                    .any(|ch| ch.key.eq_ignore_ascii_case(&rh.key))
            })
            .map(|h| h.key.clone())
            .collect();

        let req    = request_with(AuthConfig::None, req_headers.clone());
        let result = HeaderMergerService::merge(&req, &col_headers, &[]);

        // Semua col-only keys harus ada dalam output.
        for key in &col_only_keys {
            let found = count_headers_by_key(&result.headers, key);
            prop_assert_eq!(
                found,
                1,
                "collection-only header '{}' must be present in merge output",
                key
            );
        }

        // Semua req-only keys harus ada dalam output.
        for key in &req_only_keys {
            let found = count_headers_by_key(&result.headers, key);
            prop_assert_eq!(
                found,
                1,
                "request-only header '{}' must be present in merge output",
                key
            );
        }
    }

    // -----------------------------------------------------------------------
    // Property 16b — Request menimpa collection default untuk key yang sama
    // -----------------------------------------------------------------------

    /// Ketika collection default dan request mempunyai header dengan key yang
    /// sama, output harus menggunakan nilai dari request (request overrides
    /// collection), dan hanya ada satu header untuk key itu.
    #[test]
    fn prop_request_header_overrides_collection_default_for_same_key(
        key           in arbitrary_header_key(),
        col_value     in arbitrary_header_value(),
        req_value     in arbitrary_header_value(),
        other_headers in arbitrary_enabled_headers(3),
    ) {
        // Pastikan other_headers tidak mempunyai key yang sama dengan `key`.
        prop_assume!(
            !other_headers
                .iter()
                .any(|h| h.key.eq_ignore_ascii_case(&key))
        );
        // Nilai harus berbeda agar assertion bermakna.
        prop_assume!(col_value != req_value);

        let col_defaults = vec![Header {
            key:     key.clone(),
            value:   col_value.clone(),
            enabled: true,
        }];

        let mut req_headers = other_headers.clone();
        req_headers.push(Header {
            key:     key.clone(),
            value:   req_value.clone(),
            enabled: true,
        });

        let req    = request_with(AuthConfig::None, req_headers);
        let result = HeaderMergerService::merge(&req, &col_defaults, &[]);

        // Tepat satu header dengan key tersebut.
        let count = count_headers_by_key(&result.headers, &key);
        prop_assert_eq!(
            count,
            1,
            "must have exactly one '{}' header",
            key
        );

        // Nilainya dari request, bukan dari collection.
        let value = find_header_value(&result.headers, &key).unwrap();
        prop_assert_eq!(
            value,
            req_value.as_str(),
            "request value must override collection default for key '{}'",
            key
        );

        // Header lain yang tidak berkonflik tetap ada.
        for extra in &other_headers {
            let found = count_headers_by_key(&result.headers, &extra.key);
            prop_assert_eq!(
                found,
                1,
                "non-conflicting header '{}' must be preserved",
                extra.key
            );
        }
    }

    // -----------------------------------------------------------------------
    // Property 16c — Auth None tidak menghapus header manapun
    // -----------------------------------------------------------------------

    /// Dengan AuthConfig::None, setiap header yang diaktifkan dari collection
    /// default dan request harus ada dalam output.  Output count harus sama
    /// dengan jumlah unique keys dari kedua sumber.
    #[test]
    fn prop_auth_none_does_not_remove_any_enabled_header(
        col_headers in arbitrary_enabled_headers(5),
        req_headers in arbitrary_enabled_headers(5),
    ) {
        // Hitung unique keys yang diharapkan ada di output.
        let mut expected_keys: Vec<String> = Vec::new();
        for h in &col_headers {
            if !expected_keys.iter().any(|k: &String| k.eq_ignore_ascii_case(&h.key)) {
                expected_keys.push(h.key.clone());
            }
        }
        for h in &req_headers {
            if !expected_keys.iter().any(|k: &String| k.eq_ignore_ascii_case(&h.key)) {
                expected_keys.push(h.key.clone());
            }
        }
        let expected_count = expected_keys.len();

        let req    = request_with(AuthConfig::None, req_headers);
        let result = HeaderMergerService::merge(&req, &col_headers, &[]);

        // Output harus mempunyai tepat sebanyak unique key yang diharapkan.
        prop_assert_eq!(
            result.headers.len(),
            expected_count,
            "output must contain exactly {} headers (unique keys from both sources)",
            expected_count
        );

        // Setiap expected key harus ada.
        for key in &expected_keys {
            let found = count_headers_by_key(&result.headers, key);
            prop_assert_eq!(
                found,
                1,
                "expected key '{}' must appear exactly once in output",
                key
            );
        }
    }

    // -----------------------------------------------------------------------
    // Property 16d — Auth tidak menghapus header yang tidak berkonflik dengannya
    // -----------------------------------------------------------------------

    /// Ketika BearerToken digunakan, semua header collection default dan request
    /// yang keynya bukan `Authorization` tetap harus ada dalam output.
    #[test]
    fn prop_bearer_auth_does_not_drop_non_authorization_headers(
        token       in arbitrary_token(),
        col_headers in arbitrary_enabled_headers(4),
        req_headers in arbitrary_enabled_headers(4),
    ) {
        // Pastikan tidak ada `Authorization` header di kedua sumber agar
        // assertion fokus pada non-conflicting headers.
        prop_assume!(
            !col_headers
                .iter()
                .any(|h| h.key.eq_ignore_ascii_case("Authorization"))
        );
        prop_assume!(
            !req_headers
                .iter()
                .any(|h| h.key.eq_ignore_ascii_case("Authorization"))
        );

        // Hitung unique non-Authorization keys yang diharapkan.
        let mut expected_keys: Vec<String> = Vec::new();
        for h in col_headers.iter().chain(req_headers.iter()) {
            if !expected_keys.iter().any(|k: &String| k.eq_ignore_ascii_case(&h.key)) {
                expected_keys.push(h.key.clone());
            }
        }
        let expected_count = expected_keys.len();

        let auth = AuthConfig::BearerToken { token };
        let req  = request_with(auth, req_headers);
        let result = HeaderMergerService::merge(&req, &col_headers, &[]);

        // Semua non-Authorization expected keys harus ada.
        for key in &expected_keys {
            let found = count_headers_by_key(&result.headers, key);
            prop_assert_eq!(
                found,
                1,
                "non-conflicting header '{}' must be present after BearerToken injection",
                key
            );
        }

        // `Authorization` dari BearerToken harus ada.
        let auth_count = count_headers_by_key(&result.headers, "Authorization");
        prop_assert_eq!(
            auth_count,
            1,
            "Authorization from BearerToken must be present"
        );

        // Total output headers = unique non-auth keys + 1 Authorization.
        prop_assert_eq!(
            result.headers.len(),
            expected_count + 1,
            "output must have {} headers total",
            expected_count + 1
        );
    }

    // -----------------------------------------------------------------------
    // Property 16e — Disabled headers tidak ikut dalam output
    // -----------------------------------------------------------------------

    /// Header yang di-disable (enabled = false) dari collection default atau
    /// request tidak boleh muncul dalam output, bahkan jika keynya unik.
    #[test]
    fn prop_disabled_headers_are_excluded_from_output(
        key   in arbitrary_header_key(),
        value in arbitrary_header_value(),
        from_collection in proptest::bool::ANY,
    ) {
        let disabled = Header {
            key:     key.clone(),
            value:   value.clone(),
            enabled: false,
        };

        let (col_headers, req_headers): (Vec<Header>, Vec<Header>) = if from_collection {
            (vec![disabled], vec![])
        } else {
            (vec![], vec![disabled])
        };

        let req    = request_with(AuthConfig::None, req_headers);
        let result = HeaderMergerService::merge(&req, &col_headers, &[]);

        let found = count_headers_by_key(&result.headers, &key);
        prop_assert_eq!(
            found,
            0,
            "disabled header '{}' must not appear in merge output",
            key
        );
    }

    // -----------------------------------------------------------------------
    // Property 16f — ApiKey QueryParam tidak mempengaruhi header output
    // -----------------------------------------------------------------------

    /// Ketika ApiKey menggunakan lokasi QueryParam, tidak ada header yang
    /// ditambah atau dihapus — hanya extra_query_params yang bertambah.
    /// Semua headers dari collection default dan request tetap dipertahankan.
    #[test]
    fn prop_api_key_query_param_does_not_affect_headers(
        api_key_name  in arbitrary_api_key_name(),
        api_key_value in arbitrary_header_value(),
        col_headers   in arbitrary_enabled_headers(4),
        req_headers   in arbitrary_enabled_headers(4),
    ) {
        // Pastikan api_key_name tidak ada di col maupun req headers.
        prop_assume!(
            !col_headers
                .iter()
                .any(|h| h.key.eq_ignore_ascii_case(&api_key_name))
        );
        prop_assume!(
            !req_headers
                .iter()
                .any(|h| h.key.eq_ignore_ascii_case(&api_key_name))
        );

        // Hitung expected unique keys dari kedua sumber.
        let mut expected_keys: Vec<String> = Vec::new();
        for h in col_headers.iter().chain(req_headers.iter()) {
            if !expected_keys.iter().any(|k: &String| k.eq_ignore_ascii_case(&h.key)) {
                expected_keys.push(h.key.clone());
            }
        }
        let expected_count = expected_keys.len();

        let auth = AuthConfig::ApiKey {
            key:      api_key_name.clone(),
            value:    api_key_value.clone(),
            location: ApiKeyLocation::QueryParam,
        };
        let req    = request_with(auth, req_headers);
        let result = HeaderMergerService::merge(&req, &col_headers, &[]);

        // Jumlah output headers sama persis dengan expected_keys (tidak ada
        // header auth ditambahkan untuk QueryParam location).
        prop_assert_eq!(
            result.headers.len(),
            expected_count,
            "ApiKey QueryParam must not add any header to output"
        );

        // Semua expected keys ada.
        for key in &expected_keys {
            let found = count_headers_by_key(&result.headers, key);
            prop_assert_eq!(
                found,
                1,
                "header '{}' must be present in output",
                key
            );
        }

        // api_key_name tidak muncul sebagai header.
        let api_key_in_headers = count_headers_by_key(&result.headers, &api_key_name);
        prop_assert_eq!(
            api_key_in_headers,
            0,
            "api key '{}' must not appear as a header when location is QueryParam",
            api_key_name
        );

        // extra_query_params harus berisi satu entri.
        prop_assert_eq!(
            result.extra_query_params.len(),
            1,
            "extra_query_params must have exactly one entry for ApiKey QueryParam"
        );
        prop_assert_eq!(
            &result.extra_query_params[0].0,
            &api_key_name,
            "extra_query_params key must equal api_key_name"
        );
        prop_assert_eq!(
            &result.extra_query_params[0].1,
            &api_key_value,
            "extra_query_params value must equal api_key_value"
        );
    }
}
