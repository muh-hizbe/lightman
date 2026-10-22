// Property-based tests for name length constraints (Property 1).
//
// Property 1: Name Length Constraint
//   - Collection.name, Request.name, Environment.name masing-masing harus
//     1–255 karakter (non-empty, max 255).
//   - String dalam rentang valid harus lolos validasi.
//   - String kosong atau lebih dari 255 karakter harus gagal validasi.
//
// Validates: Requirements 2.1, 2.2, 3.1
// Task: 3.2

use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Validator helper — mirrors logika yang akan dipakai di services layer.
// Diletakkan di sini agar test tidak bergantung pada implementasi services
// yang belum selesai.
// ---------------------------------------------------------------------------

/// Kembalikan `true` jika `name` memenuhi constraint 1–255 karakter Unicode.
///
/// Constraint dihitung berdasarkan jumlah *karakter* (Unicode scalar values),
/// bukan byte length, agar konsisten dengan apa yang user lihat di UI.
fn is_valid_name(name: &str) -> bool {
    let len = name.chars().count();
    len >= 1 && len <= 255
}

// ---------------------------------------------------------------------------
// Property tests
// ---------------------------------------------------------------------------

proptest! {
    // --- Valid names (1–255 chars) harus selalu lolos ----------------------

    /// Setiap string dengan panjang 1–255 karakter dianggap nama yang valid.
    #[test]
    fn prop_valid_name_passes(name in ".{1,255}") {
        prop_assert!(
            is_valid_name(&name),
            "nama '{}' (len={}) seharusnya valid",
            name,
            name.len()
        );
    }

    /// Nama dengan panjang tepat 255 karakter harus valid (boundary).
    #[test]
    fn prop_name_at_max_boundary_passes(
        // Hasilkan string ASCII 255 karakter — ASCII single-byte chars sehingga
        // byte length == char count.
        name in proptest::string::string_regex("[a-zA-Z0-9 ]{255}").unwrap()
    ) {
        prop_assert_eq!(name.chars().count(), 255);
        prop_assert!(is_valid_name(&name));
    }

    /// Nama dengan panjang tepat 1 karakter harus valid (boundary).
    #[test]
    fn prop_name_at_min_boundary_passes(
        name in proptest::string::string_regex("[a-zA-Z0-9]{1}").unwrap()
    ) {
        prop_assert_eq!(name.chars().count(), 1);
        prop_assert!(is_valid_name(&name));
    }

    // --- Invalid names harus gagal ----------------------------------------

    /// String kosong selalu invalid.
    #[test]
    fn prop_empty_name_fails(_dummy in 0u8..255u8) {
        prop_assert!(!is_valid_name(""));
    }

    /// String yang lebih dari 255 karakter selalu invalid.
    #[test]
    fn prop_name_exceeding_max_fails(
        // Panjang antara 256–512
        extra in 1usize..=257usize,
    ) {
        let name = "a".repeat(255 + extra);
        prop_assert!(
            !is_valid_name(&name),
            "nama panjang {} seharusnya tidak valid",
            name.len()
        );
    }

    // --- Konsistensi: is_valid_name idempoten -----------------------------

    /// Memanggil is_valid_name dua kali pada input yang sama menghasilkan
    /// hasil yang sama (fungsi murni tanpa side-effect).
    #[test]
    fn prop_validation_is_deterministic(name in ".*") {
        let first  = is_valid_name(&name);
        let second = is_valid_name(&name);
        prop_assert_eq!(first, second);
    }
}

// ---------------------------------------------------------------------------
// Unit tests tambahan untuk edge cases spesifik
// ---------------------------------------------------------------------------

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn empty_string_is_invalid() {
        assert!(!is_valid_name(""));
    }

    #[test]
    fn single_char_is_valid() {
        assert!(is_valid_name("a"));
        assert!(is_valid_name(" "));
        assert!(is_valid_name("1"));
    }

    #[test]
    fn exactly_255_chars_is_valid() {
        let name = "x".repeat(255);
        assert!(is_valid_name(&name));
    }

    #[test]
    fn exactly_256_chars_is_invalid() {
        let name = "x".repeat(256);
        assert!(!is_valid_name(&name));
    }

    #[test]
    fn unicode_multibyte_chars_counted_by_char_count() {
        // "é" adalah 2 bytes di UTF-8 tetapi 1 karakter.
        // 255 kopi "é" = 255 karakter → valid.
        let name = "é".repeat(255);
        assert_eq!(name.chars().count(), 255);
        assert!(is_valid_name(&name));

        // 256 kopi "é" = 256 karakter → invalid.
        let too_long = "é".repeat(256);
        assert_eq!(too_long.chars().count(), 256);
        assert!(!is_valid_name(&too_long));

        // Karakter 4-byte (emoji) juga dihitung sebagai 1 karakter.
        let emoji_name = "🚀".repeat(255);
        assert_eq!(emoji_name.chars().count(), 255);
        assert!(is_valid_name(&emoji_name));
    }

    #[test]
    fn typical_collection_names_are_valid() {
        let names = [
            "My API Collection",
            "Production Environment",
            "GitHub REST API v3",
            "Test Suite — Auth Flow",
        ];
        for name in &names {
            assert!(is_valid_name(name), "'{name}' seharusnya valid");
        }
    }

    #[test]
    fn whitespace_only_name_passes_length_check() {
        // Validasi panjang saja tidak menolak whitespace-only;
        // logika bisnis bisa menambahkan trim di layer service.
        assert!(is_valid_name("   "));
    }
}
