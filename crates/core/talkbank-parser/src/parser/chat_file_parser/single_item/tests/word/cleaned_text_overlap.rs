//! Test module for cleaned text with overlap markers in `talkbank-chat`.
//!
//! CHAT principle: contiguous non-whitespace is always one word. Overlap
//! markers (⌈⌉⌊⌋, optionally followed by a digit like ⌋2 or ⌈3) follow
//! this rule — when adjacent to word text (no space), they are part of the
//! word token. When space-separated, they are standalone overlap_point tokens.
//!
//! The cleaned_text() method strips overlap markers (including their optional
//! digit) from word text. They are structural annotations indicating where
//! overlapping speech begins and ends — not spoken text.

use super::helpers::{parse_word, snapshot};

/// Overlap markers (with digits) are stripped; only spoken text remains.
#[test]
fn overlap_marker_simple_in_word() {
    // "Yeah⌋⌈2:" — ⌋ and ⌈2 are overlap markers, : is lengthening
    let result = parse_word("Yeah⌋⌈2:");

    if let Ok(word) = &result {
        assert_eq!(word.cleaned_text(), "Yeah");
    }

    snapshot(
        "word_parsing_tests__overlap_marker_simple_excluded_from_cleaned_text",
        &result,
    );
}

/// Mid-word overlap markers (with digits) are stripped.
#[test]
fn overlap_marker_midword() {
    // "a⌋2⌈3side" — ⌋2 and ⌈3 are overlap markers
    let result = parse_word("a⌋2⌈3side");

    if let Ok(word) = &result {
        assert_eq!(word.cleaned_text(), "aside");
    }

    snapshot(
        "word_parsing_tests__overlap_marker_midword_excluded",
        &result,
    );
}

/// Overlap markers and lengthening both stripped from cleaned text.
#[test]
fn overlap_marker_complex() {
    // "ye⌉2⌊3:s" — ⌉2 and ⌊3 are overlap markers, : is lengthening
    let result = parse_word("ye⌉2⌊3:s");

    if let Ok(word) = &result {
        assert_eq!(word.cleaned_text(), "yes");
    }

    snapshot(
        "word_parsing_tests__overlap_marker_complex_excluded",
        &result,
    );
}

/// Leading overlap marker is stripped from cleaned text.
#[test]
fn overlap_marker_at_beginning() {
    let result = parse_word("⌊hello");

    if let Ok(word) = &result {
        assert_eq!(word.cleaned_text(), "hello");
    }

    snapshot("word_parsing_tests__overlap_marker_at_beginning", &result);
}

/// Numbered overlap markers are stripped; spoken text between them is kept.
#[test]
fn overlap_marker_numbered() {
    // "test⌋1⌈2ing" — ⌋1 and ⌈2 are overlap markers
    let result = parse_word("test⌋1⌈2ing");

    if let Ok(word) = &result {
        assert_eq!(word.cleaned_text(), "testing");
    }

    snapshot(
        "word_parsing_tests__overlap_marker_numbered_excluded",
        &result,
    );
}

/// All overlap marker variants are stripped from cleaned text.
#[test]
fn overlap_marker_all_types() {
    let result = parse_word("w⌈or⌉d⌊te⌋st");

    if let Ok(word) = &result {
        assert_eq!(word.cleaned_text(), "wordtest");
    }

    snapshot(
        "word_parsing_tests__overlap_marker_all_types_excluded",
        &result,
    );
}
