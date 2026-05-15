//! Snapshot tests for word validation error messages.
//!
//! These tests use insta to snapshot test the error messages produced by validation.
//! This ensures error messages remain consistent and any changes are intentional.

use crate::content::word::WordCompoundMarker;
use crate::model::{
    CADelimiter, CADelimiterType, LanguageCode, Word, WordContent, WordLengthening,
    WordStressMarker, WordStressMarkerType, WordSyllablePause, WordText,
};
use crate::validation::{Validate, ValidationContext};
use crate::{ErrorCollector, ParseError};

/// Format validation errors into snapshot-friendly lines.
///
/// The output stays intentionally compact so snapshot diffs highlight semantic
/// diagnostic changes instead of formatting noise.
fn format_errors(errors: &[ParseError]) -> String {
    if errors.is_empty() {
        "[No errors]".to_string()
    } else {
        errors
            .iter()
            .map(|e| format!("[{}] {} - {}", e.code.as_str(), e.severity, e.message))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Executes word validation with a configured context and collected error sink.
///
/// This helper centralizes context setup so individual snapshots can focus on
/// one token pattern at a time.
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

/// Snapshots diagnostics for a leading compound marker (`E232`).
#[test]
fn snapshot_e232_compound_marker_at_start() {
    let word = Word::new_unchecked("+word", "word").with_content(vec![
        WordContent::CompoundMarker(WordCompoundMarker::new()),
        WordContent::Text(WordText::new_unchecked("word")),
    ]);
    let errors = run_word_validation(&word, None, &[], false);
    insta::assert_snapshot!(format_errors(&errors));
}

/// Snapshots diagnostics for a trailing compound marker (`E232`/`E233` path).
#[test]
fn snapshot_e232_compound_marker_at_end() {
    let word = Word::new_unchecked("word+", "word").with_content(vec![
        WordContent::Text(WordText::new_unchecked("word")),
        WordContent::CompoundMarker(WordCompoundMarker::new()),
    ]);
    let errors = run_word_validation(&word, None, &[], false);
    insta::assert_snapshot!(format_errors(&errors));
}

/// Snapshots the clean case for a well-formed compound word.
#[test]
fn snapshot_valid_compound_word_no_errors() {
    let word = Word::new_unchecked("ice+cream", "ice+cream");
    let errors = run_word_validation(&word, None, &[], false);
    insta::assert_snapshot!(format_errors(&errors));
}

/// Snapshots diagnostics for a word containing a lengthening marker.
#[test]
fn snapshot_word_lengthening_validation() {
    let word = Word::new_unchecked("wo:rd", "wo:rd").with_content(vec![
        WordContent::Text(WordText::new_unchecked("wo")),
        WordContent::Lengthening(WordLengthening::new()),
        WordContent::Text(WordText::new_unchecked("rd")),
    ]);

    let errors = run_word_validation(&word, None, &[], false);
    insta::assert_snapshot!(format_errors(&errors));
}

/// Snapshots diagnostics for primary stress marker placement.
#[test]
fn snapshot_stress_marker_validation() {
    let word = Word::new_unchecked("WOrd", "WOrd").with_content(vec![
        WordContent::StressMarker(WordStressMarker::new(WordStressMarkerType::Primary)),
        WordContent::Text(WordText::new_unchecked("O")),
        WordContent::Text(WordText::new_unchecked("rd")),
    ]);

    let errors = run_word_validation(&word, None, &[], false);
    insta::assert_snapshot!(format_errors(&errors));
}

/// Snapshots diagnostics for syllable-pause marker placement.
#[test]
fn snapshot_syllable_pause_validation() {
    let word = Word::new_unchecked("wo^rd", "wo^rd").with_content(vec![
        WordContent::Text(WordText::new_unchecked("wo")),
        WordContent::SyllablePause(WordSyllablePause::new()),
        WordContent::Text(WordText::new_unchecked("rd")),
    ]);

    let errors = run_word_validation(&word, None, &[], false);
    insta::assert_snapshot!(format_errors(&errors));
}

/// Snapshots the balanced nested-CA-delimiter case.
#[test]
fn snapshot_nested_ca_delimiters_balanced() {
    let word = Word::new_unchecked("°∆fast∆°", "fast").with_content(vec![
        WordContent::CADelimiter(CADelimiter::new(CADelimiterType::Softer)),
        WordContent::CADelimiter(CADelimiter::new(CADelimiterType::Faster)),
        WordContent::Text(WordText::new_unchecked("fast")),
        WordContent::CADelimiter(CADelimiter::new(CADelimiterType::Faster)),
        WordContent::CADelimiter(CADelimiter::new(CADelimiterType::Softer)),
    ]);

    let errors = run_word_validation(&word, None, &[], false);
    insta::assert_snapshot!(format_errors(&errors));
}
