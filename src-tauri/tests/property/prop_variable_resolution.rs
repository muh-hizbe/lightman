// Property-based tests untuk variable resolution (Properties 5, 6, 7).
//
// Task 5.2 — tiga property divalidasi:
//
//   Property 5: Interpolation Identity on Placeholder-Free Strings
//     Jika input string tidak mengandung pola `{{...}}`, hasil interpolasi
//     harus identik dengan input — tidak ada karakter yang berubah.
//     Validates: Requirement 3.2
//
//   Property 6: Interpolation Completeness for Present Variables
//     Setiap placeholder `{{key}}` yang key-nya ada di lookup harus
//     diganti sepenuhnya — tidak boleh ada `{{key}}` tersisa dalam output.
//     Validates: Requirement 3.2
//
//   Property 7: Interpolation Preserves Unresolved Placeholders
//     Placeholder `{{key}}` yang key-nya tidak ada di lookup harus tetap
//     ada dalam output persis seperti semula — tidak boleh diubah atau dihapus.
//     Validates: Requirement 3.5

use proptest::prelude::*;
use lightman_lib::models::{
    AuthConfig, Header, HttpMethod, QueryParam, Request, RequestBody, Variable,
};
use lightman_lib::services::var_interpolator::VarInterpolatorService;

// ---------------------------------------------------------------------------
// Strategies
// ---------------------------------------------------------------------------

/// Identifier valid untuk variable key: huruf kapital, angka, underscore,
/// antara 1–20 karakter. Tidak dimulai dengan angka.
fn valid_var_key() -> impl Strategy<Value = String> {
    proptest::string::string_regex("[A-Z][A-Z0-9_]{0,19}")
        .expect("valid_var_key regex should compile")
}

/// Variable value arbitrary — bisa berisi karakter apa pun kecuali `{{` dan
/// `}}` agar tidak menciptakan placeholder bersarang yang mempersulit verifikasi.
fn safe_var_value() -> impl Strategy<Value = String> {
    proptest::string::string_regex("[A-Za-z0-9 ._\\-/]{0,40}")
        .expect("safe_var_value regex should compile")
}

/// String yang dijamin tidak mengandung substring `{{`.
/// Digunakan untuk Property 5.
fn placeholder_free_string() -> impl Strategy<Value = String> {
    proptest::string::string_regex("[A-Za-z0-9 ._\\-/:@#?&=+%]{0,80}")
        .expect("placeholder_free_string regex should compile")
}

/// Buat satu `Variable` enabled dengan key dan value yang diberikan.
fn make_var(key: impl Into<String>, value: impl Into<String>) -> Variable {
    Variable {
        id: uuid::Uuid::new_v4().to_string(),
        environment_id: None,
        key: key.into(),
        value: value.into(),
        enabled: true,
    }
}

/// Buat `Request` minimal dengan URL yang diberikan.
fn base_request(url: impl Into<String>) -> Request {
    Request {
        id: uuid::Uuid::new_v4().to_string(),
        collection_id: "col-1".to_string(),
        name: "prop-test".to_string(),
        method: HttpMethod::Get,
        url: url.into(),
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
// Helper: tokio runtime
// ---------------------------------------------------------------------------

// (Tidak diperlukan — VarInterpolatorService::interpolate adalah fungsi sync.)

// ---------------------------------------------------------------------------
// Property 5 — Interpolation Identity on Placeholder-Free Strings
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// String tanpa `{{` tidak boleh diubah oleh interpolasi, terlepas dari
    /// variable apa pun yang ada di lookup.
    #[test]
    fn prop_identity_on_placeholder_free_url(
        url   in placeholder_free_string(),
        key   in valid_var_key(),
        value in safe_var_value(),
    ) {
        // Pastikan URL benar-benar bebas placeholder.
        prop_assume!(!url.contains("{{"));

        let vars = vec![make_var(&key, &value)];
        let req  = base_request(url.clone());

        let out = VarInterpolatorService::interpolate(req, &vars, &[], &[]);

        prop_assert_eq!(
            &out.url,
            &url,
            "URL without placeholders must be unchanged after interpolation"
        );
    }

    /// Header value tanpa `{{` tidak boleh diubah.
    #[test]
    fn prop_identity_on_placeholder_free_header_value(
        value in placeholder_free_string(),
        key   in valid_var_key(),
        var_v in safe_var_value(),
    ) {
        prop_assume!(!value.contains("{{"));

        let vars = vec![make_var(&key, &var_v)];
        let mut req = base_request("https://example.com");
        req.headers = vec![Header {
            key: "X-Custom".to_string(),
            value: value.clone(),
            enabled: true,
        }];

        let out = VarInterpolatorService::interpolate(req, &vars, &[], &[]);

        prop_assert_eq!(
            &out.headers[0].value,
            &value,
            "header value without placeholders must be unchanged"
        );
    }

    /// Query param value tanpa `{{` tidak boleh diubah.
    #[test]
    fn prop_identity_on_placeholder_free_param_value(
        value in placeholder_free_string(),
        key   in valid_var_key(),
        var_v in safe_var_value(),
    ) {
        prop_assume!(!value.contains("{{"));

        let vars = vec![make_var(&key, &var_v)];
        let mut req = base_request("https://example.com");
        req.params = vec![QueryParam {
            key: "q".to_string(),
            value: value.clone(),
            enabled: true,
        }];

        let out = VarInterpolatorService::interpolate(req, &vars, &[], &[]);

        prop_assert_eq!(
            &out.params[0].value,
            &value,
            "param value without placeholders must be unchanged"
        );
    }
}

// ---------------------------------------------------------------------------
// Property 6 — Interpolation Completeness for Present Variables
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Placeholder `{{KEY}}` yang ada di lookup harus hilang dari output URL.
    #[test]
    fn prop_completeness_url_placeholder_replaced(
        key   in valid_var_key(),
        value in safe_var_value(),
        prefix in placeholder_free_string(),
        suffix in placeholder_free_string(),
    ) {
        // Bangun URL: <prefix>{{KEY}}<suffix>
        let url  = format!("{}{{{{{}}}}}{}",  prefix, key, suffix);
        let vars = vec![make_var(&key, &value)];
        let req  = base_request(url.clone());

        let out  = VarInterpolatorService::interpolate(req, &vars, &[], &[]);

        // Placeholder tidak boleh lagi hadir dalam output.
        let placeholder = format!("{{{{{}}}}}", key);
        prop_assert!(
            !out.url.contains(&placeholder),
            "placeholder '{placeholder}' must not remain in URL after interpolation"
        );

        // Nilai variabel harus hadir dalam output (kecuali value kosong).
        if !value.is_empty() {
            prop_assert!(
                out.url.contains(&value),
                "resolved value '{value}' must appear in URL after interpolation"
            );
        }
    }

    /// Placeholder di header value yang ada di lookup harus diganti sepenuhnya.
    #[test]
    fn prop_completeness_header_placeholder_replaced(
        key   in valid_var_key(),
        value in safe_var_value(),
        prefix in placeholder_free_string(),
        suffix in placeholder_free_string(),
    ) {
        let header_val  = format!("{}{{{{{}}}}}{}",  prefix, key, suffix);
        let vars        = vec![make_var(&key, &value)];
        let mut req     = base_request("https://example.com");
        req.headers = vec![Header {
            key: "Authorization".to_string(),
            value: header_val.clone(),
            enabled: true,
        }];

        let out         = VarInterpolatorService::interpolate(req, &vars, &[], &[]);
        let placeholder = format!("{{{{{}}}}}", key);

        prop_assert!(
            !out.headers[0].value.contains(&placeholder),
            "placeholder '{placeholder}' must not remain in header value after interpolation"
        );
    }

    /// Placeholder di query param value yang ada di lookup harus diganti.
    #[test]
    fn prop_completeness_param_placeholder_replaced(
        key   in valid_var_key(),
        value in safe_var_value(),
        prefix in placeholder_free_string(),
    ) {
        let param_val   = format!("{}{{{{{}}}}}",  prefix, key);
        let vars        = vec![make_var(&key, &value)];
        let mut req     = base_request("https://example.com");
        req.params = vec![QueryParam {
            key: "filter".to_string(),
            value: param_val.clone(),
            enabled: true,
        }];

        let out         = VarInterpolatorService::interpolate(req, &vars, &[], &[]);
        let placeholder = format!("{{{{{}}}}}", key);

        prop_assert!(
            !out.params[0].value.contains(&placeholder),
            "placeholder '{placeholder}' must not remain in param value after interpolation"
        );
    }

    /// Interpolasi JSON body — placeholder yang ada di lookup harus hilang.
    #[test]
    fn prop_completeness_json_body_placeholder_replaced(
        key   in valid_var_key(),
        value in safe_var_value(),
    ) {
        let body_content = format!("{{\"id\":\"{{{{{}}}}}\"}}",  key);
        let vars         = vec![make_var(&key, &value)];
        let mut req      = base_request("https://example.com");
        req.body = RequestBody::Json {
            content: body_content.clone(),
        };

        let out         = VarInterpolatorService::interpolate(req, &vars, &[], &[]);
        let placeholder = format!("{{{{{}}}}}", key);

        if let RequestBody::Json { content } = &out.body {
            prop_assert!(
                !content.contains(&placeholder),
                "placeholder '{placeholder}' must not remain in JSON body after interpolation"
            );
        } else {
            prop_assert!(false, "body must remain RequestBody::Json after interpolation");
        }
    }

    /// Priority: env_vars menang atas collection_vars dan global_vars.
    #[test]
    fn prop_completeness_env_priority_over_collection_and_global(
        key          in valid_var_key(),
        env_value    in safe_var_value(),
        col_value    in safe_var_value(),
        global_value in safe_var_value(),
    ) {
        // Semua scope memiliki key yang sama — env_value harus menang.
        prop_assume!(env_value != col_value);
        prop_assume!(env_value != global_value);
        prop_assume!(!env_value.is_empty());

        let env_var    = make_var(&key, &env_value);
        let col_var    = make_var(&key, &col_value);
        let global_var = make_var(&key, &global_value);

        let url = format!("https://example.com/{{{{{}}}}}",  key);
        let req = base_request(url);

        let out = VarInterpolatorService::interpolate(req, &[env_var], &[col_var], &[global_var]);

        prop_assert!(
            out.url.contains(&env_value),
            "env value '{env_value}' must win over collection/global values"
        );
        prop_assert!(
            !out.url.contains(&format!("{{{{{}}}}}", key)),
            "placeholder must be resolved"
        );
    }
}

// ---------------------------------------------------------------------------
// Property 7 — Interpolation Preserves Unresolved Placeholders
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Placeholder `{{KEY}}` yang key-nya tidak ada di lookup harus tetap ada
    /// dalam output persis seperti semula.
    #[test]
    fn prop_unresolved_placeholder_preserved_in_url(
        missing_key in valid_var_key(),
        other_key   in valid_var_key(),
        other_value in safe_var_value(),
        prefix      in placeholder_free_string(),
        suffix      in placeholder_free_string(),
    ) {
        // Pastikan missing_key berbeda dari other_key agar tidak ter-resolve.
        prop_assume!(missing_key != other_key);

        let placeholder = format!("{{{{{}}}}}", missing_key);
        let url         = format!("{}{}{}",  prefix, placeholder, suffix);

        // Hanya other_key yang ada di lookup — missing_key tidak ada.
        let vars = vec![make_var(&other_key, &other_value)];
        let req  = base_request(url.clone());

        let out  = VarInterpolatorService::interpolate(req, &vars, &[], &[]);

        prop_assert!(
            out.url.contains(&placeholder),
            "unresolved placeholder '{placeholder}' must be preserved in URL"
        );
    }

    /// Placeholder yang tidak ada di lookup harus tetap ada di header value.
    #[test]
    fn prop_unresolved_placeholder_preserved_in_header(
        missing_key in valid_var_key(),
        other_key   in valid_var_key(),
        other_value in safe_var_value(),
    ) {
        prop_assume!(missing_key != other_key);

        let placeholder  = format!("{{{{{}}}}}", missing_key);
        let header_value = format!("Bearer {}",  placeholder);

        let vars     = vec![make_var(&other_key, &other_value)];
        let mut req  = base_request("https://example.com");
        req.headers  = vec![Header {
            key:     "Authorization".to_string(),
            value:   header_value.clone(),
            enabled: true,
        }];

        let out = VarInterpolatorService::interpolate(req, &vars, &[], &[]);

        prop_assert!(
            out.headers[0].value.contains(&placeholder),
            "unresolved placeholder '{placeholder}' must be preserved in header value"
        );
    }

    /// Placeholder yang tidak ada di lookup harus tetap ada di param value.
    #[test]
    fn prop_unresolved_placeholder_preserved_in_param(
        missing_key in valid_var_key(),
        other_key   in valid_var_key(),
        other_value in safe_var_value(),
    ) {
        prop_assume!(missing_key != other_key);

        let placeholder = format!("{{{{{}}}}}", missing_key);
        let param_value = placeholder.clone();

        let vars     = vec![make_var(&other_key, &other_value)];
        let mut req  = base_request("https://example.com");
        req.params   = vec![QueryParam {
            key:     "id".to_string(),
            value:   param_value.clone(),
            enabled: true,
        }];

        let out = VarInterpolatorService::interpolate(req, &vars, &[], &[]);

        prop_assert!(
            out.params[0].value.contains(&placeholder),
            "unresolved placeholder '{placeholder}' must be preserved in param value"
        );
    }

    /// Disabled variables tidak masuk lookup — placeholder yang hanya ada di
    /// disabled var harus tetap tidak ter-resolve.
    #[test]
    fn prop_disabled_variable_leaves_placeholder_unresolved(
        key   in valid_var_key(),
        value in safe_var_value(),
    ) {
        prop_assume!(!value.is_empty());

        let placeholder = format!("{{{{{}}}}}", key);
        let url         = format!("https://example.com/{}",  placeholder);

        // Variabel DISABLED — tidak boleh dipakai untuk interpolasi.
        let disabled_var = Variable {
            id:             uuid::Uuid::new_v4().to_string(),
            environment_id: None,
            key:            key.clone(),
            value:          value.clone(),
            enabled:        false, // <-- disabled
        };

        let req = base_request(url);
        let out = VarInterpolatorService::interpolate(req, &[disabled_var], &[], &[]);

        prop_assert!(
            out.url.contains(&placeholder),
            "disabled variable must not resolve placeholder '{placeholder}'"
        );
    }

    /// String dengan banyak placeholder — yang ada di lookup di-resolve,
    /// yang tidak ada harus tetap ada.
    #[test]
    fn prop_mixed_placeholders_partial_resolution(
        present_key   in valid_var_key(),
        present_value in safe_var_value(),
        absent_key    in valid_var_key(),
    ) {
        prop_assume!(present_key != absent_key);

        let present_ph = format!("{{{{{}}}}}", present_key);
        let absent_ph  = format!("{{{{{}}}}}", absent_key);
        let url        = format!("https://example.com/{}/{}",  present_ph, absent_ph);

        let vars = vec![make_var(&present_key, &present_value)];
        let req  = base_request(url);

        let out  = VarInterpolatorService::interpolate(req, &vars, &[], &[]);

        // Present placeholder harus tidak ada lagi.
        prop_assert!(
            !out.url.contains(&present_ph),
            "present placeholder '{present_ph}' must be resolved"
        );

        // Absent placeholder harus tetap ada.
        prop_assert!(
            out.url.contains(&absent_ph),
            "absent placeholder '{absent_ph}' must be preserved"
        );
    }
}
