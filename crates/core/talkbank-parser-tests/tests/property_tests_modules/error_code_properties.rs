//! Property-based tests for `ErrorCode` enum invariants.
//!
//! These tests verify that the proc-macro-generated methods on `ErrorCode`
//! maintain their contracts: roundtrip through `as_str`/`new`, display
//! consistency, and documentation URL structure.

use proptest::prelude::*;
use talkbank_model::ErrorCode;

/// All known error codes as a static slice.
///
/// This list is maintained manually. If a new error code is added to the enum,
/// add it here. The `error_code_roundtrip` test will catch any code that fails
/// to round-trip, but this list ensures we test a representative sample.
///
/// We use a hand-curated subset covering each error code range (E0xx through
/// E9xx) to avoid duplicating the entire enum, while still exercising the
/// proc-macro-generated methods across all ranges.
const SAMPLE_CODES: &[ErrorCode] = &[
    // E0xx — generic/internal
    ErrorCode::InternalError,
    ErrorCode::TestError,
    ErrorCode::EmptyString,
    // E1xx — structural/file
    ErrorCode::InvalidLineFormat,
    // E2xx — word errors
    ErrorCode::MissingFormType,
    ErrorCode::InvalidFormType,
    ErrorCode::UnknownAnnotation,
    // E3xx — parser errors
    ErrorCode::MissingMainTier,
    ErrorCode::MissingNode,
    ErrorCode::SyntaxError,
    ErrorCode::MissingSpeaker,
    ErrorCode::MissingTerminator,
    ErrorCode::EmptyUtterance,
    ErrorCode::UnexpectedSyntax,
    ErrorCode::ParseFailed,
    ErrorCode::UnexpectedNode,
    // E3xx — tier parse errors
    ErrorCode::MorParseError,
    // E999 — unknown (fallback)
    ErrorCode::UnknownError,
];

/// Strategy that selects a random code from the sample set.
fn arb_error_code() -> impl Strategy<Value = ErrorCode> {
    (0..SAMPLE_CODES.len()).prop_map(|i| SAMPLE_CODES[i])
}

proptest! {
    /// `ErrorCode::new(code.as_str())` round-trips back to the same variant.
    ///
    /// The proc macro generates `as_str` and `new` from the same mapping table,
    /// so every known code must survive a full round-trip. `UnknownError` is
    /// included in the test — it maps to `"E999"` and back.
    #[test]
    fn error_code_roundtrip(code in arb_error_code()) {
        let code_str = code.as_str();
        let reconstructed = ErrorCode::new(code_str);
        prop_assert_eq!(
            reconstructed, code,
            "Round-trip failed: {:?}.as_str() = {:?}, new({:?}) = {:?}",
            code, code_str, code_str, reconstructed
        );
    }

    /// `Display` output matches `as_str()` for every error code.
    ///
    /// The proc macro generates `Display` to delegate to `as_str`, so these
    /// must always agree. This catches any drift if the macro is refactored.
    #[test]
    fn error_code_display_matches_as_str(code in arb_error_code()) {
        let display = format!("{}", code);
        let as_str = code.as_str();
        prop_assert_eq!(
            &display, as_str,
            "Display ({:?}) != as_str ({:?}) for {:?}",
            display, as_str, code
        );
    }

    /// `documentation_url()` contains the error code string.
    ///
    /// The URL is constructed as `https://talkbank.org/errors/{code}`, so the
    /// code string must appear in the URL. This catches malformed URL templates.
    #[test]
    fn error_code_documentation_url_contains_code(code in arb_error_code()) {
        let url = code.documentation_url();
        let code_str = code.as_str();
        prop_assert!(
            url.contains(code_str),
            "documentation_url ({:?}) should contain code {:?}",
            url, code_str
        );
    }
}
