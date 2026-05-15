//! Regression tests for word-level validation rules.
//!
//! These fixtures pin down the intended behavior of `Word` validators at common
//! boundary conditions: compound boundaries, whitespace/formatting contamination,
//! prosodic marker placement, and language-aware token checks.
//!
//! CHAT references:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Words>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Word_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Compounds>
//! - <https://talkbank.org/0info/manuals/CHAT.html#PrimaryStress_Element>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Lengthening_Marker>
//! - <https://talkbank.org/0info/manuals/CHAT.html#WordInternalPause_Marker>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Language_Switching>

use crate::content::word::WordCompoundMarker;
use crate::model::{
    LanguageCode, Word, WordContent, WordContents, WordLengthening, WordStressMarker,
    WordStressMarkerType, WordSyllablePause, WordText,
};
use crate::validation::{Validate, ValidationContext};
use crate::{ErrorCollector, ParseError, Span};
use serde_json;

/// Validates one word under a caller-controlled language/CA context.
///
/// Tests use this helper to keep fixtures focused on token structure instead of
/// repeating `ValidationContext` and sink wiring.
fn run_word_validation(
    word: &Word,
    tier_language: Option<&LanguageCode>,
    declared_languages: &[LanguageCode],
    ca_mode: bool,
) -> Vec<ParseError> {
    let context = ValidationContext::new()
        .with_declared_languages(declared_languages.to_vec())
        .with_ca_mode(ca_mode)
        .with_tier_language(tier_language.cloned());
    let errors = ErrorCollector::new();
    word.validate(&context, &errors);
    errors.into_vec()
}

/// Rejects compounds that start with `+` and therefore have no left segment.
///
/// This protects the `E232` invariant and also checks that reported spans stay
/// within the source word bounds.
#[test]
fn test_compound_marker_at_start() {
    let word = Word::new_unchecked("+word", "word")
        .with_content(vec![
            WordContent::CompoundMarker(WordCompoundMarker::new()),
            WordContent::Text(WordText::new_unchecked("word")),
        ])
        .with_span(Span::from(10..15));

    let errors = run_word_validation(&word, None, &[], false);

    let e232_errors: Vec<_> = errors
        .iter()
        .filter(|e| e.code.as_str() == "E232")
        .collect();
    assert!(!e232_errors.is_empty(), "Leading + should produce E232");

    // Verify error spans are within word bounds
    for error in &e232_errors {
        let span = &error.location.span;
        assert!(
            span.start >= 10 && span.end <= 15,
            "E232 error span should be within word bounds [10, 15], got [{}, {}]",
            span.start,
            span.end
        );
    }
}

/// Rejects compounds that end with `+` and therefore have no right segment.
///
/// The validator must emit `E233` for trailing compound markers.
#[test]
fn test_compound_marker_at_end() {
    let word = Word::new_unchecked("word+", "word").with_content(vec![
        WordContent::Text(WordText::new_unchecked("word")),
        WordContent::CompoundMarker(WordCompoundMarker::new()),
    ]);

    let errors = run_word_validation(&word, None, &[], false);

    assert!(
        errors.iter().any(|e| e.code.as_str() == "E233"),
        "Trailing + should produce E233"
    );
}

/// Rejects adjacent `++` markers that create an empty compound part.
///
/// This fixture ensures internal empty segments are treated the same as trailing
/// empties and reported as `E233`.
#[test]
fn test_compound_marker_empty_parts() {
    let word = Word::new_unchecked("un++do", "undo").with_content(vec![
        WordContent::Text(WordText::new_unchecked("un")),
        WordContent::CompoundMarker(WordCompoundMarker::new()),
        WordContent::CompoundMarker(WordCompoundMarker::new()),
        WordContent::Text(WordText::new_unchecked("do")),
    ]);

    let errors = run_word_validation(&word, None, &[], false);

    assert!(
        errors.iter().any(|e| e.code.as_str() == "E233"),
        "Empty compound parts ++ should produce E233"
    );
}

/// Accepts well-formed compounds with non-empty parts on both sides of `+`.
///
/// A valid `un+do` token should not trigger `E232` or `E233`.
#[test]
fn test_valid_compound() {
    let word = Word::new_unchecked("un+do", "un+do");

    let errors = run_word_validation(&word, None, &[], false);

    assert!(
        !errors
            .iter()
            .any(|e| e.code.as_str() == "E232" || e.code.as_str() == "E233"),
        "Valid compound should not produce compound errors"
    );
}

/// Flags embedded digits in languages where word tokens must be alphabetic.
///
/// With `eng` context, `hello123` should emit `E220` and keep the error span
/// within the token span.
#[test]
fn test_e220_digit_validation_eng() {
    // "hello123" - 8 bytes
    let word = Word::new_unchecked("hello123", "hello123").with_span(Span::from(20..28));
    let errors = run_word_validation(
        &word,
        Some(&LanguageCode::new("eng")),
        &[LanguageCode::new("eng")],
        false,
    );

    let e220_errors: Vec<_> = errors
        .iter()
        .filter(|e| e.code.as_str() == "E220")
        .collect();
    assert!(
        !e220_errors.is_empty(),
        "Expected E220 error for digits in English word, got: {:#?}",
        errors
    );

    // Verify error spans are within word bounds
    for error in &e220_errors {
        let span = &error.location.span;
        assert!(
            span.start >= 20 && span.end <= 28,
            "E220 error span should be within word bounds [20, 28], got [{}, {}]",
            span.start,
            span.end
        );
    }
}

/// Allows digits in language contexts that explicitly permit numeral forms.
///
/// Chinese (`zho`) is one of the configured exceptions and should not emit
/// `E220` for mixed alphanumeric tokens.
#[test]
fn test_e220_digit_validation_zho() {
    let word = Word::new_unchecked("hello123", "hello123");
    let errors = run_word_validation(
        &word,
        Some(&LanguageCode::new("zho")),
        &[LanguageCode::new("zho")],
        false,
    );

    assert!(
        !errors.iter().any(|e| e.code.as_str() == "E220"),
        "E220 should not trigger for Chinese (zho) - digits are allowed"
    );
}

/// Leaves digit policy disabled when no language context is available.
///
/// This avoids over-eager `E220` reports when a tier/header has not declared
/// language constraints.
#[test]
fn test_e220_no_language_context() {
    let word = Word::new_unchecked("hello123", "hello123");
    let errors = run_word_validation(&word, None, &[], false);

    assert!(
        !errors.iter().any(|e| e.code.as_str() == "E220"),
        "E220 should not trigger when no language in context"
    );
}

/// Confirms all configured digit-allowing languages bypass `E220`.
///
/// This table test guards against regressions when language lists are edited.
#[test]
fn test_e220_all_number_languages() {
    let number_langs = vec!["zho", "cym", "vie", "tha", "nan", "yue", "min", "hak"];
    let word = Word::new_unchecked("word123", "word123");

    for lang in number_langs {
        let lang_code = LanguageCode::new(lang);
        let errors = run_word_validation(
            &word,
            Some(&lang_code),
            std::slice::from_ref(&lang_code),
            false,
        );
        assert!(
            !errors.iter().any(|e| e.code.as_str() == "E220"),
            "E220 should not trigger for {} - digits allowed",
            lang
        );
    }
}

/// Normalizes placeholder conventions by rejecting lowercase `xx`.
///
/// `E241` should be emitted with guidance toward canonical `xxx`.
#[test]
fn test_e241_illegal_xx() {
    let word = Word::new_unchecked("xx", "xx");
    let errors = run_word_validation(&word, None, &[], false);

    assert!(
        errors
            .iter()
            .any(|e| e.code.as_str() == "E241" && e.message.contains("xxx")),
        "Expected E241 error suggesting 'xxx' for 'xx', got: {:#?}",
        errors
    );
}

/// Rejects uppercase placeholder variants that deviate from CHAT casing.
///
/// `XXX` should still map to `E241` with a suggestion for lowercase `xxx`.
#[test]
fn test_e241_illegal_xxx() {
    let word = Word::new_unchecked("XXX", "XXX");
    let errors = run_word_validation(&word, None, &[], false);

    assert!(
        errors
            .iter()
            .any(|e| e.code.as_str() == "E241" && e.message.contains("xxx")),
        "Expected E241 error suggesting 'xxx' for 'XXX', got: {:#?}",
        errors
    );
}

/// Rejects non-canonical two-letter uncertain forms like `yy`.
///
/// The validator should steer users to canonical `yyy` via `E241`.
#[test]
fn test_e241_illegal_yy() {
    let word = Word::new_unchecked("yy", "yy");
    let errors = run_word_validation(&word, None, &[], false);

    assert!(
        errors
            .iter()
            .any(|e| e.code.as_str() == "E241" && e.message.contains("yyy")),
        "Expected E241 error suggesting 'yyy' for 'yy', got: {:#?}",
        errors
    );
}

/// Rejects uppercase `WWW` in favor of canonical lowercase `www`.
///
/// This keeps reserved token conventions stable for downstream tooling.
#[test]
fn test_e241_illegal_www() {
    let word = Word::new_unchecked("WWW", "WWW");
    let errors = run_word_validation(&word, None, &[], false);

    assert!(
        errors
            .iter()
            .any(|e| e.code.as_str() == "E241" && e.message.contains("www")),
        "Expected E241 error suggesting 'www' for 'WWW', got: {:#?}",
        errors
    );
}

/// Accepts canonical placeholder tokens such as lowercase `xxx`.
///
/// This guards against false-positive `E241` errors on valid forms.
#[test]
fn test_e241_valid_xxx() {
    let word = Word::new_unchecked("xxx", "xxx");
    let errors = run_word_validation(&word, None, &[], false);

    assert!(
        !errors.iter().any(|e| e.code.as_str() == "E241"),
        "Valid 'xxx' should not trigger E241"
    );
}

/// Flags trailing whitespace contamination in word tokens.
///
/// This case mirrors historical `%wor` export issues where trailing spaces were
/// accidentally serialized into token text.
#[test]
fn test_e243_word_with_trailing_space() {
    // This catches the %wor tier bug where words include trailing spaces
    let word = Word::new_unchecked("hello ", "hello ");
    let errors = run_word_validation(&word, None, &[], false);

    assert!(
        errors.iter().any(|e| e.code.as_str() == "E243"),
        "Expected E243 error for word with trailing space, got: {:#?}",
        errors
    );
}

/// Flags leading whitespace contamination in word tokens.
///
/// Leading spaces should consistently emit `E243` rather than silently passing.
#[test]
fn test_e243_word_with_leading_space() {
    let word = Word::new_unchecked(" hello", " hello");
    let errors = run_word_validation(&word, None, &[], false);

    assert!(
        errors.iter().any(|e| e.code.as_str() == "E243"),
        "Expected E243 error for word with leading space, got: {:#?}",
        errors
    );
}

/// Rejects stray media bullet bytes inside lexical word text.
///
/// The U+0015 marker is structural metadata, so textual inclusion should report
/// `E243` with a bullet-specific message.
#[test]
fn test_e243_word_with_bullet_marker() {
    // Bullet marker U+0015 (byte 0x15) should never be in word text
    let word_with_bullet = "hello\x15".to_string();
    let word = Word::new_unchecked(&word_with_bullet, &word_with_bullet);
    let errors = run_word_validation(&word, None, &[], false);

    assert!(
        errors
            .iter()
            .any(|e| e.code.as_str() == "E243" && e.message.contains("bullet")),
        "Expected E243 error for word with bullet marker, got: {:#?}",
        errors
    );
}

/// Rejects tab characters embedded in a word token.
///
/// Tabs are treated as formatting contamination and should emit `E243`.
#[test]
fn test_e243_word_with_tab() {
    let word = Word::new_unchecked("hello\tworld", "hello\tworld");
    let errors = run_word_validation(&word, None, &[], false);

    assert!(
        errors.iter().any(|e| e.code.as_str() == "E243"),
        "Expected E243 error for word with tab character, got: {:#?}",
        errors
    );
}

/// Rejects newline characters embedded in a word token.
///
/// Newlines must not survive tokenization and therefore trigger `E243`.
#[test]
fn test_e243_word_with_newline() {
    let word = Word::new_unchecked("hello\nworld", "hello\nworld");
    let errors = run_word_validation(&word, None, &[], false);

    assert!(
        errors.iter().any(|e| e.code.as_str() == "E243"),
        "Expected E243 error for word with newline, got: {:#?}",
        errors
    );
}

/// Confirms clean lexical tokens do not spuriously emit formatting errors.
///
/// A plain token should pass with no `E243`.
#[test]
fn test_e243_clean_word_no_error() {
    let word = Word::new_unchecked("hello", "hello");
    let errors = run_word_validation(&word, None, &[], false);

    assert!(
        !errors.iter().any(|e| e.code.as_str() == "E243"),
        "Clean word should not trigger E243"
    );
}

/// Handles mixed contamination (`space + bullet`) in one token robustly.
///
/// This regression fixture checks that at least one `E243` is raised for the
/// real-world `%wor` corruption pattern.
#[test]
fn test_e243_word_with_space_and_bullet() {
    // This is the exact pattern from %wor tier bug: "word \x15"
    let word_with_space_and_bullet = "hello \x15".to_string();
    let word = Word::new_unchecked(&word_with_space_and_bullet, &word_with_space_and_bullet);
    let errors = run_word_validation(&word, None, &[], false);

    // Should get at least one E243 error (maybe two - one for space, one for bullet)
    let e243_errors: Vec<_> = errors
        .iter()
        .filter(|e| e.code.as_str() == "E243")
        .collect();
    assert!(
        !e243_errors.is_empty(),
        "Expected E243 error for word with space and bullet marker, got: {:#?}",
        errors
    );
}

// =============================================================================
// Prosodic Marker Validation Tests (E244-E247, E250, E251)
// =============================================================================

/// Rejects adjacent stress markers with no intervening segment text.
///
/// Consecutive stress elements should emit `E244` because stress must attach to
/// an actual syllabic segment.
#[test]
fn test_e244_consecutive_stress_markers() {
    // ˈˌtest - two stress markers in a row
    let word = Word::new_unchecked("ˈˌtest", "test").with_content(vec![
        WordContent::StressMarker(WordStressMarker::new(WordStressMarkerType::Primary)),
        WordContent::StressMarker(WordStressMarker::new(WordStressMarkerType::Secondary)),
        WordContent::Text(WordText::new_unchecked("test")),
    ]);
    let errors = run_word_validation(&word, None, &[], false);

    assert!(
        errors.iter().any(|e| e.code.as_str() == "E244"),
        "Expected E244 error for consecutive stress markers, got: {:#?}",
        errors
    );
}

/// Accepts multiple stress markers when separated by lexical text.
///
/// This ensures `E244` is limited to truly adjacent markers, not repeated stress
/// across distinct subparts.
#[test]
fn test_e244_valid_non_consecutive_stress() {
    // ˈsyl·ˌla·ble - stress markers separated by text
    let word = Word::new_unchecked("ˈsylˌlable", "syllable").with_content(vec![
        WordContent::StressMarker(WordStressMarker::new(WordStressMarkerType::Primary)),
        WordContent::Text(WordText::new_unchecked("syl")),
        WordContent::StressMarker(WordStressMarker::new(WordStressMarkerType::Secondary)),
        WordContent::Text(WordText::new_unchecked("lable")),
    ]);
    let errors = run_word_validation(&word, None, &[], false);

    assert!(
        !errors.iter().any(|e| e.code.as_str() == "E244"),
        "Non-consecutive stress markers should not trigger E244"
    );
}

/// Rejects stress markers that appear at the end of a word token.
///
/// A trailing stress mark has no following segment to modify and should emit
/// `E245`.
#[test]
fn test_e245_stress_at_word_end() {
    // testˈ - stress marker at end with no following text
    let word = Word::new_unchecked("testˈ", "test").with_content(vec![
        WordContent::Text(WordText::new_unchecked("test")),
        WordContent::StressMarker(WordStressMarker::new(WordStressMarkerType::Primary)),
    ]);
    let errors = run_word_validation(&word, None, &[], false);

    assert!(
        errors.iter().any(|e| e.code.as_str() == "E245"),
        "Expected E245 error for stress at word end, got: {:#?}",
        errors
    );
}

/// Accepts stress markers that precede lexical text.
///
/// This positive case keeps `E245` scoped to dangling trailing markers.
#[test]
fn test_e245_valid_stress_before_text() {
    // ˈtest - stress marker followed by text
    let word = Word::new_unchecked("ˈtest", "test").with_content(vec![
        WordContent::StressMarker(WordStressMarker::new(WordStressMarkerType::Primary)),
        WordContent::Text(WordText::new_unchecked("test")),
    ]);
    let errors = run_word_validation(&word, None, &[], false);

    assert!(
        !errors.iter().any(|e| e.code.as_str() == "E245"),
        "Stress before text should not trigger E245"
    );
}

/// Rejects lengthening markers that appear before any base segment text.
///
/// CHAT lengthening marks must follow a segment, so leading `:` should emit
/// `E246`.
#[test]
fn test_e246_lengthening_at_word_start() {
    // :test - colon at start with no preceding text
    let word = Word::new_unchecked(":test", "test").with_content(vec![
        WordContent::Lengthening(WordLengthening::new()),
        WordContent::Text(WordText::new_unchecked("test")),
    ]);
    let errors = run_word_validation(&word, None, &[], false);

    assert!(
        errors.iter().any(|e| e.code.as_str() == "E246"),
        "Expected E246 error for lengthening at word start, got: {:#?}",
        errors
    );
}

/// Accepts lengthening markers placed after lexical text.
///
/// This confirms valid in-word placement does not trigger `E246`.
#[test]
fn test_e246_valid_lengthening_after_text() {
    // ba:nana - colon after text
    let word = Word::new_unchecked("ba:nana", "banana").with_content(vec![
        WordContent::Text(WordText::new_unchecked("ba")),
        WordContent::Lengthening(WordLengthening::new()),
        WordContent::Text(WordText::new_unchecked("nana")),
    ]);
    let errors = run_word_validation(&word, None, &[], false);

    assert!(
        !errors.iter().any(|e| e.code.as_str() == "E246"),
        "Lengthening after text should not trigger E246"
    );
}

/// Rejects words containing more than one primary stress marker.
///
/// Primary stress is constrained to one occurrence per token and violations emit
/// `E247`.
#[test]
fn test_e247_multiple_primary_stress() {
    // ˈtestˈword - two primary stress markers
    let word = Word::new_unchecked("ˈtestˈword", "testword").with_content(vec![
        WordContent::StressMarker(WordStressMarker::new(WordStressMarkerType::Primary)),
        WordContent::Text(WordText::new_unchecked("test")),
        WordContent::StressMarker(WordStressMarker::new(WordStressMarkerType::Primary)),
        WordContent::Text(WordText::new_unchecked("word")),
    ]);
    let errors = run_word_validation(&word, None, &[], false);

    assert!(
        errors.iter().any(|e| e.code.as_str() == "E247"),
        "Expected E247 error for multiple primary stress markers, got: {:#?}",
        errors
    );
}

/// Accepts words with exactly one primary stress marker.
///
/// This positive fixture prevents regressions that would over-report `E247`.
#[test]
fn test_e247_valid_single_primary_stress() {
    // ˈtest - only one primary stress
    let word = Word::new_unchecked("ˈtest", "test").with_content(vec![
        WordContent::StressMarker(WordStressMarker::new(WordStressMarkerType::Primary)),
        WordContent::Text(WordText::new_unchecked("test")),
    ]);
    let errors = run_word_validation(&word, None, &[], false);

    assert!(
        !errors.iter().any(|e| e.code.as_str() == "E247"),
        "Single primary stress should not trigger E247"
    );
}

/// Rejects secondary stress when no primary stress exists in the token.
///
/// `E250` enforces the dependency between secondary and primary stress markers.
#[test]
fn test_e250_secondary_without_primary() {
    // ˌtest - secondary stress without primary
    let word = Word::new_unchecked("ˌtest", "test").with_content(vec![
        WordContent::StressMarker(WordStressMarker::new(WordStressMarkerType::Secondary)),
        WordContent::Text(WordText::new_unchecked("test")),
    ]);
    let errors = run_word_validation(&word, None, &[], false);

    assert!(
        errors.iter().any(|e| e.code.as_str() == "E250"),
        "Expected E250 error for secondary stress without primary, got: {:#?}",
        errors
    );
}

/// Accepts secondary stress when a primary stress marker is also present.
///
/// This keeps `E250` focused on missing-primary violations only.
#[test]
fn test_e250_valid_secondary_with_primary() {
    // ˈtestˌword - secondary stress with primary present
    let word = Word::new_unchecked("ˈtestˌword", "testword").with_content(vec![
        WordContent::StressMarker(WordStressMarker::new(WordStressMarkerType::Primary)),
        WordContent::Text(WordText::new_unchecked("test")),
        WordContent::StressMarker(WordStressMarker::new(WordStressMarkerType::Secondary)),
        WordContent::Text(WordText::new_unchecked("word")),
    ]);
    let errors = run_word_validation(&word, None, &[], false);

    assert!(
        !errors.iter().any(|e| e.code.as_str() == "E250"),
        "Secondary stress with primary present should not trigger E250"
    );
}

/// Rejects words whose parsed content list is unexpectedly empty.
///
/// `E253` protects downstream logic that assumes each word has at least one
/// semantic element.
#[test]
fn test_e253_empty_word_content_list() {
    let mut word = Word::new_unchecked("test", "test");
    word.content = WordContents::default();
    let errors = run_word_validation(&word, None, &[], false);

    assert!(
        errors.iter().any(|e| e.code.as_str() == "E253"),
        "Expected E253 error for empty word content list, got: {:#?}",
        errors
    );
}

/// Confirms non-empty content vectors pass the `E253` structural check.
///
/// A normal `Word` should preserve at least one element in `content`.
#[test]
fn test_e253_non_empty_word_content_list() {
    let word = Word::new_unchecked("test", "test");
    let errors = run_word_validation(&word, None, &[], false);

    assert!(
        !errors.iter().any(|e| e.code.as_str() == "E253"),
        "Non-empty word content list should not trigger E253"
    );
}

/// Rejects empty `WordText` elements inside an otherwise populated word.
///
/// This catches silent empty segments that can break alignment and morphology
/// indexing, and should emit `E251`.
#[test]
fn test_e251_empty_word_content_text() -> Result<(), String> {
    // Word with empty Text content
    let empty_text: WordText = serde_json::from_str("\"\"")
        .map_err(|err| format!("Failed to deserialize empty WordText: {err}"))?;
    let word = Word::new_unchecked("test", "test").with_content(vec![
        WordContent::Text(empty_text),
        WordContent::Text(WordText::new_unchecked("test")),
    ]);
    let errors = run_word_validation(&word, None, &[], false);

    assert!(
        errors.iter().any(|e| e.code.as_str() == "E251"),
        "Expected E251 error for empty word content text, got: {:#?}",
        errors
    );
    Ok(())
}

/// Accepts `WordText` elements with non-empty lexical content.
///
/// This positive case prevents false-positive `E251` reports.
#[test]
fn test_e251_valid_non_empty_text() {
    // Word with non-empty Text content
    let word = Word::new_unchecked("test", "test")
        .with_content(vec![WordContent::Text(WordText::new_unchecked("test"))]);
    let errors = run_word_validation(&word, None, &[], false);

    assert!(
        !errors.iter().any(|e| e.code.as_str() == "E251"),
        "Non-empty word content text should not trigger E251"
    );
}

/// Rejects syllable-pause markers that appear before any segment text.
///
/// Leading `^` has no left syllable boundary to annotate and should emit `E252`.
#[test]
fn test_e252_syllable_pause_at_word_start() {
    // ^test - caret at start with no preceding text
    let word = Word::new_unchecked("^test", "test").with_content(vec![
        WordContent::SyllablePause(WordSyllablePause::new()),
        WordContent::Text(WordText::new_unchecked("test")),
    ]);
    let errors = run_word_validation(&word, None, &[], false);

    assert!(
        errors.iter().any(|e| e.code.as_str() == "E252"),
        "Expected E252 error for syllable pause at word start, got: {:#?}",
        errors
    );
}

/// Rejects syllable-pause markers that appear after all segment text.
///
/// Trailing `^` has no following syllable and should emit `E252`.
#[test]
fn test_e252_syllable_pause_at_word_end() {
    // test^ - caret at end with no following text
    let word = Word::new_unchecked("test^", "test").with_content(vec![
        WordContent::Text(WordText::new_unchecked("test")),
        WordContent::SyllablePause(WordSyllablePause::new()),
    ]);
    let errors = run_word_validation(&word, None, &[], false);

    assert!(
        errors.iter().any(|e| e.code.as_str() == "E252"),
        "Expected E252 error for syllable pause at word end, got: {:#?}",
        errors
    );
}

/// Accepts syllable-pause markers placed between lexical segments.
///
/// Interior pause markers are valid CHAT word-internal boundaries and must not
/// trigger `E252`.
#[test]
fn test_e252_valid_syllable_pause_between_text() {
    // rhi^noceros - caret between syllables
    let word = Word::new_unchecked("rhi^noceros", "rhinoceros").with_content(vec![
        WordContent::Text(WordText::new_unchecked("rhi")),
        WordContent::SyllablePause(WordSyllablePause::new()),
        WordContent::Text(WordText::new_unchecked("noceros")),
    ]);
    let errors = run_word_validation(&word, None, &[], false);

    assert!(
        !errors.iter().any(|e| e.code.as_str() == "E252"),
        "Syllable pause between text should not trigger E252"
    );
}
