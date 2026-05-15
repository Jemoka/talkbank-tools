//! ISO 639-3 language code registry lookup.
//!
//! Provides compile-time perfect hash set of all ~8,367 ISO 639-3 language
//! codes, generated from `clan-info/lib/fixes/ISO 639-3.txt` by `build.rs`.
//!
//! Used by `LanguageCode::validate()` to check membership.

// Include the generated phf::Set from build.rs.
include!(concat!(env!("OUT_DIR"), "/iso639_3_set.rs"));

/// Returns `true` if the ISO 639-3 code set was populated at build time.
///
/// Returns `false` when the vendored `data/iso639-3.txt` was missing at build
/// time (should not happen in normal circumstances — the file is committed
/// to the repo). Tests that require a populated set can use this as a guard.
pub fn iso639_3_set_available() -> bool {
    !ISO_639_3_CODES.is_empty()
}

/// Check whether a 3-letter code is a valid ISO 639-3 language code.
///
/// Returns `true` if the code is in the official registry. Returns `true`
/// when the set is empty (graceful degradation — see `iso639_3_set_available`).
pub fn is_valid_iso639_3(code: &str) -> bool {
    if !iso639_3_set_available() {
        // Empty set means the data file wasn't available at build time.
        // Degrade gracefully: don't reject any codes.
        return true;
    }
    ISO_639_3_CODES.contains(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_codes_are_valid() {
        assert!(is_valid_iso639_3("eng"));
        assert!(is_valid_iso639_3("spa"));
        assert!(is_valid_iso639_3("zho"));
        assert!(is_valid_iso639_3("fra"));
        assert!(is_valid_iso639_3("deu"));
        assert!(is_valid_iso639_3("jpn"));
        assert!(is_valid_iso639_3("yue")); // Cantonese
        assert!(is_valid_iso639_3("cym")); // Welsh
    }

    #[test]
    fn invalid_codes_are_rejected() {
        assert!(!is_valid_iso639_3("cye")); // not in ISO 639-3
        assert!(!is_valid_iso639_3("tze")); // not in ISO 639-3
        assert!(!is_valid_iso639_3("zzz")); // not a real code
        assert!(!is_valid_iso639_3("xyz")); // placeholder
    }

    #[test]
    fn technically_valid_but_suspicious_codes() {
        // These ARE in ISO 639-3 but are unlikely in TalkBank context.
        // They may be typos for common codes but are not our job to reject.
        assert!(is_valid_iso639_3("nle")); // East Nyala (probably meant nld)
        assert!(is_valid_iso639_3("enh")); // Tundra Enets (probably meant eng)
        assert!(is_valid_iso639_3("ena")); // Apali (probably meant eng)
    }

    #[test]
    fn set_is_populated() {
        // The ISO 639-3 data is vendored in data/iso639-3.txt — always present.
        assert!(
            iso639_3_set_available(),
            "ISO 639-3 set should be populated (data/iso639-3.txt missing?)"
        );
    }

    #[test]
    fn set_has_expected_size() {
        // The official ISO 639-3 registry has ~8,367 codes.
        let count = ISO_639_3_CODES.len();
        assert!(count > 8000, "Expected 8000+ codes, got {}", count);
    }
}
