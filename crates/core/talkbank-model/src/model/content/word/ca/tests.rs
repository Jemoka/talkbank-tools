//! Tests for word-internal Conversation Analysis symbols.
//!
//! The suite validates glyph mapping stability and serialization fidelity for
//! both CA elements and paired delimiter symbol sets.

use super::*;
use crate::Span;
use crate::generated::symbol_sets::{CA_DELIMITER_SYMBOLS, CA_ELEMENT_SYMBOLS};
use crate::model::WriteChat;
use std::collections::BTreeSet;

/// CA element variants map to the expected CHAT glyphs.
///
/// This catches regressions where enum-to-symbol mapping drifts from spec.
#[test]
fn test_ca_element_to_symbol() {
    assert_eq!(CAElementType::PitchUp.to_symbol(), "↑");
    assert_eq!(CAElementType::PitchDown.to_symbol(), "↓");
}

/// CA delimiter variants map to the expected CHAT glyphs.
///
/// The assertions include symbols that are easy to confuse visually in Unicode.
#[test]
fn test_ca_delimiter_to_symbol() {
    assert_eq!(CADelimiterType::Faster.to_symbol(), "∆");
    assert_eq!(CADelimiterType::Slower.to_symbol(), "∇");
    assert_eq!(CADelimiterType::Softer.to_symbol(), "°");
    // ☺ is SmileVoice per tree-sitter grammar (U+263A WHITE SMILING FACE)
    assert_eq!(CADelimiterType::SmileVoice.to_symbol(), "☺");
    // ∬ is Whisper per tree-sitter grammar (U+222C DOUBLE INTEGRAL)
    assert_eq!(CADelimiterType::Whisper.to_symbol(), "∬");
}

/// Builds a CA element with default span metadata.
#[test]
fn test_ca_element_creation() {
    let elem = CAElement::new(CAElementType::PitchUp);
    assert_eq!(elem.element_type, CAElementType::PitchUp);
    assert_eq!(elem.span, None);
}

/// Preserves explicit span metadata on CA elements.
#[test]
fn test_ca_element_with_span() {
    let span = Span::new(0, 3);
    let elem = CAElement::new(CAElementType::PitchUp).with_span(span);
    assert_eq!(elem.span, Some(span));
}

/// Builds a CA delimiter with default span metadata.
#[test]
fn test_ca_delimiter_creation() {
    let delim = CADelimiter::new(CADelimiterType::Faster);
    assert_eq!(delim.delimiter_type, CADelimiterType::Faster);
    assert_eq!(delim.span, None);
}

/// Preserves explicit span metadata on CA delimiters.
#[test]
fn test_ca_delimiter_with_span() {
    let span = Span::new(5, 8);
    let delim = CADelimiter::new(CADelimiterType::Softer).with_span(span);
    assert_eq!(delim.span, Some(span));
}

/// Serializes CA elements to their CHAT glyph form.
#[test]
fn test_ca_element_write_chat() {
    let elem = CAElement::new(CAElementType::PitchUp);
    let mut output = String::new();
    let _ = elem.write_chat(&mut output);
    assert_eq!(output, "↑");
}

/// Serializes CA delimiters to their CHAT glyph form.
#[test]
fn test_ca_delimiter_write_chat() {
    let delim = CADelimiter::new(CADelimiterType::Faster);
    let mut output = String::new();
    let _ = delim.write_chat(&mut output);
    assert_eq!(output, "∆");
}

/// Ensures runtime CA element symbols stay in sync with generated registry data.
#[test]
fn test_ca_element_symbol_set_matches_generated_registry() {
    let actual: BTreeSet<&'static str> = [
        CAElementType::BlockedSegments.to_symbol(),
        CAElementType::Constriction.to_symbol(),
        CAElementType::Hardening.to_symbol(),
        CAElementType::HurriedStart.to_symbol(),
        CAElementType::Inhalation.to_symbol(),
        CAElementType::LaughInWord.to_symbol(),
        CAElementType::PitchDown.to_symbol(),
        CAElementType::PitchReset.to_symbol(),
        CAElementType::PitchUp.to_symbol(),
        CAElementType::SuddenStop.to_symbol(),
    ]
    .into_iter()
    .collect();
    let expected: BTreeSet<&'static str> = CA_ELEMENT_SYMBOLS.iter().copied().collect();
    assert_eq!(actual, expected);
}

/// Ensures runtime CA delimiter symbols stay in sync with generated registry data.
#[test]
fn test_ca_delimiter_symbol_set_matches_generated_registry() {
    let actual: BTreeSet<&'static str> = [
        CADelimiterType::Faster.to_symbol(),
        CADelimiterType::Slower.to_symbol(),
        CADelimiterType::Softer.to_symbol(),
        CADelimiterType::LowPitch.to_symbol(),
        CADelimiterType::HighPitch.to_symbol(),
        CADelimiterType::SmileVoice.to_symbol(),
        CADelimiterType::BreathyVoice.to_symbol(),
        CADelimiterType::Unsure.to_symbol(),
        CADelimiterType::Whisper.to_symbol(),
        CADelimiterType::Yawn.to_symbol(),
        CADelimiterType::Singing.to_symbol(),
        CADelimiterType::SegmentRepetition.to_symbol(),
        CADelimiterType::Creaky.to_symbol(),
        CADelimiterType::Louder.to_symbol(),
        CADelimiterType::Precise.to_symbol(),
    ]
    .into_iter()
    .collect();
    let expected: BTreeSet<&'static str> = CA_DELIMITER_SYMBOLS.iter().copied().collect();
    assert_eq!(actual, expected);
}
