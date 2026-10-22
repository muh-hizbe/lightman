// Property-based tests untuk auth header generation (Properties 13, 14).
//
// Task 6.2 — dua property divalidasi:
//
//   Property 13: Basic Auth Header Encoding
//     Untuk sembarang username dan password, header `Authorization` yang
//     dihasilkan oleh `BasicAuth` harus selalu sama dengan
//     `"Basic " + base64_standard(username + ":" + password)`.
//     Validates: Requirement 9.2
//
//   Property 14: Bearer Token Header Injection
//     Untuk sembarang token string, header `Authorization` yang dihasilkan
//     oleh `BearerToken` harus selalu sama dengan `"Bearer " + token`.
//     Validates: Requirement 9.3

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use proptest::prelude::*;

use lightman_lib::models::{AuthConfig, Header, HttpMethod, RequestBody};
use lightman_lib::models::Request;
use lightman_lib::services::header_merger::HeaderMergerService;

// ---------------------------------------------------------------------------
// Strategies
// ---------------------------------------------------------------------------

/// Arbitrary string yang bisa muncul sebagai username, password, atau token.
/// Karakter yang dipakai: printable ASCII kecuali control chars.
/// Panjang 0–64 karakter untuk menjaga kecepatan test.
fn arbitrary_credential() -> impl Strategy<Value = String> {
    proptest::string::string_regex("[A-Za-z0-9 !@#$%^&*()_+\\-=\\[\\]{}|;:',.<>?/~`]{0,64}")
        .expect("arbitrary_credential regex should compile")
}

/// Arbitrary token string — tidak mengandung spasi agar lebih realistis,
/// tetapi property tetap berlaku untuk semua nilai.
fn arbitrary_token() -> impl Strategy<Value = String> {
    proptest::string::string_regex("[A-Za-z0-9._\\-+/=]{1,128}")
        .expect("arbitrary_token regex should compile")
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Buat `Request` minimal dengan `AuthConfig` yang diberikan dan tanpa headers
/// manual, sehingga satu-satunya header dalam hasil merge berasal dari auth.
fn request_with_auth(auth: AuthConfig) -> Request {
    Request {
        id: "req-prop".to_string(),
        collection_id: "col-prop".to_string(),
        name: "prop-test".to_string(),
        method: HttpMethod::Get,
        url: "https://example.com".to_string(),
        headers: vec![],
        params: vec![],
        body: RequestBody::None,
        auth,
        position: 0,
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
    }
}

/// Ambil nilai header `Authorization` dari hasil merge, atau kembalikan `None`
/// jika header tersebut tidak ada.
fn find_authorization(headers: &[Header]) -> Option<&str> {
    headers
        .iter()
        .find(|h| h.key.eq_ignore_ascii_case("Authorization"))
        .map(|h| h.value.as_str())
}

// ---------------------------------------------------------------------------
// Property 13 — Basic Auth Header Encoding
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Untuk sembarang username dan password, nilai `Authorization` yang
    /// diinjeksi oleh `BasicAuth` harus selalu `"Basic <base64(user:pass)>"`.
    ///
    /// Sifat yang divalidasi:
    /// 1. Header `Authorization` pasti ada dalam output.
    /// 2. Nilai dimulai dengan prefix `"Basic "`.
    /// 3. Bagian setelah `"Basic "` adalah hasil base64-encode dari
    ///    string `"<username>:<password>"` menggunakan STANDARD alphabet
    ///    (tidak URL-safe, dengan padding `=`).
    #[test]
    fn prop_basic_auth_authorization_header_is_correctly_encoded(
        username in arbitrary_credential(),
        password in arbitrary_credential(),
    ) {
        let auth = AuthConfig::BasicAuth {
            username: username.clone(),
            password: password.clone(),
        };
        let req    = request_with_auth(auth);
        let result = HeaderMergerService::merge(&req, &[], &[]);

        // --- assertion 1: header harus ada ---
        let auth_value = find_authorization(&result.headers);
        prop_assert!(
            auth_value.is_some(),
            "Authorization header must be present for BasicAuth"
        );
        let auth_value = auth_value.unwrap();

        // --- assertion 2: prefix "Basic " ---
        prop_assert!(
            auth_value.starts_with("Basic "),
            "Authorization value must start with 'Basic ', got: '{auth_value}'"
        );

        // --- assertion 3: base64 correctness ---
        let encoded_part = &auth_value["Basic ".len()..];
        let expected_payload = format!("{username}:{password}");
        let expected_encoded = BASE64.encode(expected_payload.as_bytes());

        prop_assert_eq!(
            encoded_part,
            expected_encoded.as_str(),
            "{}",
            format!(
                "base64 payload must equal base64(username:password) — \
                 username='{username}', password='{password}'"
            )
        );
    }

    /// Hanya ada tepat satu header `Authorization` dalam output,
    /// bahkan ketika username atau password mengandung karakter `:`
    /// (colon hanya dipakai sebagai separator pertama dalam `user:pass`).
    #[test]
    fn prop_basic_auth_produces_exactly_one_authorization_header(
        username in arbitrary_credential(),
        password in arbitrary_credential(),
    ) {
        let auth = AuthConfig::BasicAuth {
            username: username.clone(),
            password: password.clone(),
        };
        let req    = request_with_auth(auth);
        let result = HeaderMergerService::merge(&req, &[], &[]);

        let count = result
            .headers
            .iter()
            .filter(|h| h.key.eq_ignore_ascii_case("Authorization"))
            .count();

        prop_assert_eq!(
            count,
            1,
            "there must be exactly one Authorization header for BasicAuth"
        );
    }

    /// `BasicAuth` tidak menghasilkan extra_query_params.
    #[test]
    fn prop_basic_auth_has_no_extra_query_params(
        username in arbitrary_credential(),
        password in arbitrary_credential(),
    ) {
        let auth = AuthConfig::BasicAuth {
            username: username.clone(),
            password: password.clone(),
        };
        let req    = request_with_auth(auth);
        let result = HeaderMergerService::merge(&req, &[], &[]);

        prop_assert!(
            result.extra_query_params.is_empty(),
            "BasicAuth must not produce extra_query_params"
        );
    }

    /// Basic Auth menimpa header `Authorization` manual yang sudah ada,
    /// dan hasilnya tetap mengikuti formula `Basic base64(user:pass)`.
    #[test]
    fn prop_basic_auth_overrides_manual_authorization_header(
        username     in arbitrary_credential(),
        password     in arbitrary_credential(),
        manual_value in arbitrary_credential(),
    ) {
        let auth = AuthConfig::BasicAuth {
            username: username.clone(),
            password: password.clone(),
        };

        let mut req = request_with_auth(auth);
        // Tambahkan header Authorization manual yang nilainya berbeda.
        req.headers.push(Header {
            key:     "Authorization".to_string(),
            value:   manual_value.clone(),
            enabled: true,
        });

        let result = HeaderMergerService::merge(&req, &[], &[]);

        // Hanya satu Authorization header.
        let auth_headers: Vec<_> = result
            .headers
            .iter()
            .filter(|h| h.key.eq_ignore_ascii_case("Authorization"))
            .collect();

        prop_assert_eq!(
            auth_headers.len(),
            1,
            "must be exactly one Authorization header after override"
        );

        // Nilainya harus dari BasicAuth, bukan dari header manual.
        let expected_payload = format!("{username}:{password}");
        let expected_value   = format!("Basic {}", BASE64.encode(expected_payload.as_bytes()));

        prop_assert_eq!(
            &auth_headers[0].value,
            &expected_value,
            "BasicAuth must override manual Authorization header"
        );
    }
}

// ---------------------------------------------------------------------------
// Property 14 — Bearer Token Header Injection
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Untuk sembarang token, nilai `Authorization` yang diinjeksi oleh
    /// `BearerToken` harus selalu `"Bearer <token>"` persis — tanpa encoding
    /// atau transformasi tambahan.
    ///
    /// Sifat yang divalidasi:
    /// 1. Header `Authorization` pasti ada.
    /// 2. Nilai dimulai dengan prefix `"Bearer "`.
    /// 3. Bagian setelah `"Bearer "` identik dengan token asli.
    #[test]
    fn prop_bearer_token_authorization_header_is_correctly_formatted(
        token in arbitrary_token(),
    ) {
        let auth = AuthConfig::BearerToken { token: token.clone() };
        let req    = request_with_auth(auth);
        let result = HeaderMergerService::merge(&req, &[], &[]);

        // --- assertion 1: header harus ada ---
        let auth_value = find_authorization(&result.headers);
        prop_assert!(
            auth_value.is_some(),
            "Authorization header must be present for BearerToken"
        );
        let auth_value = auth_value.unwrap();

        // --- assertion 2: prefix "Bearer " ---
        prop_assert!(
            auth_value.starts_with("Bearer "),
            "Authorization value must start with 'Bearer ', got: '{auth_value}'"
        );

        // --- assertion 3: token tidak diubah ---
        let token_part = &auth_value["Bearer ".len()..];
        prop_assert_eq!(
            token_part,
            token.as_str(),
            "token part of Authorization header must equal original token"
        );
    }

    /// Hanya ada tepat satu header `Authorization` dalam output untuk BearerToken.
    #[test]
    fn prop_bearer_token_produces_exactly_one_authorization_header(
        token in arbitrary_token(),
    ) {
        let auth = AuthConfig::BearerToken { token: token.clone() };
        let req    = request_with_auth(auth);
        let result = HeaderMergerService::merge(&req, &[], &[]);

        let count = result
            .headers
            .iter()
            .filter(|h| h.key.eq_ignore_ascii_case("Authorization"))
            .count();

        prop_assert_eq!(
            count,
            1,
            "there must be exactly one Authorization header for BearerToken"
        );
    }

    /// `BearerToken` tidak menghasilkan extra_query_params.
    #[test]
    fn prop_bearer_token_has_no_extra_query_params(
        token in arbitrary_token(),
    ) {
        let auth = AuthConfig::BearerToken { token: token.clone() };
        let req    = request_with_auth(auth);
        let result = HeaderMergerService::merge(&req, &[], &[]);

        prop_assert!(
            result.extra_query_params.is_empty(),
            "BearerToken must not produce extra_query_params"
        );
    }

    /// BearerToken menimpa header `Authorization` manual yang sudah ada,
    /// dan hasilnya tetap `"Bearer <token>"`.
    #[test]
    fn prop_bearer_token_overrides_manual_authorization_header(
        token        in arbitrary_token(),
        manual_value in arbitrary_credential(),
    ) {
        let auth = AuthConfig::BearerToken { token: token.clone() };

        let mut req = request_with_auth(auth);
        req.headers.push(Header {
            key:     "Authorization".to_string(),
            value:   manual_value.clone(),
            enabled: true,
        });

        let result = HeaderMergerService::merge(&req, &[], &[]);

        let auth_headers: Vec<_> = result
            .headers
            .iter()
            .filter(|h| h.key.eq_ignore_ascii_case("Authorization"))
            .collect();

        prop_assert_eq!(
            auth_headers.len(),
            1,
            "must be exactly one Authorization header after override"
        );

        let expected = format!("Bearer {token}");
        prop_assert_eq!(
            &auth_headers[0].value,
            &expected,
            "BearerToken must override manual Authorization header"
        );
    }

    /// Collection default headers tidak mempengaruhi nilai Authorization dari
    /// BearerToken — token injection tetap benar apapun default headers-nya.
    #[test]
    fn prop_bearer_token_correct_with_collection_default_headers(
        token            in arbitrary_token(),
        col_header_key   in proptest::string::string_regex("[A-Za-z][A-Za-z0-9-]{0,20}")
                             .expect("col_header_key regex"),
        col_header_value in arbitrary_credential(),
    ) {
        // Pastikan collection default header bukan Authorization
        // agar tidak menjadi confounding factor pada assertion utama.
        prop_assume!(!col_header_key.eq_ignore_ascii_case("Authorization"));

        let auth = AuthConfig::BearerToken { token: token.clone() };
        let req  = request_with_auth(auth);

        let collection_defaults = vec![Header {
            key:     col_header_key,
            value:   col_header_value,
            enabled: true,
        }];

        let result = HeaderMergerService::merge(&req, &collection_defaults, &[]);

        let auth_value = find_authorization(&result.headers)
            .expect("Authorization header must be present");

        let expected = format!("Bearer {token}");
        prop_assert_eq!(
            auth_value,
            expected.as_str(),
            "Bearer token injection must be correct regardless of collection default headers"
        );
    }
}
