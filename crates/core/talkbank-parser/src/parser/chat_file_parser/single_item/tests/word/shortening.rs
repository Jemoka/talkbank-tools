//! Test module for shortening in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use super::helpers::{parse_word, snapshot};

/// Parses a single parenthesized shortening and preserves the expanded cleaned text.
#[test]
fn word_with_shortening() {
    let result = parse_word("hel(lo)");

    if let Ok(word) = &result {
        assert_eq!(word.raw_text(), "hel(lo)");
        assert_eq!(word.cleaned_text(), "hello");
        assert!(!word.content.is_empty());
    }

    snapshot("word_parsing_tests__word_with_shortening", &result);
}

/// Parses multiple shortening spans within one lexical token.
#[test]
fn word_with_multiple_shortenings() {
    let result = parse_word("h(e)l(lo)");

    if let Ok(word) = &result {
        assert_eq!(word.raw_text(), "h(e)l(lo)");
        assert_eq!(word.cleaned_text(), "hello");
        assert!(word.content.len() >= 2);
    }

    snapshot(
        "word_parsing_tests__word_with_multiple_shortenings",
        &result,
    );
}

/// Rejects unclosed shortening spans.
#[test]
fn error_unclosed_shortening() {
    let result = parse_word("hel(lo");

    if let Err(errors) = &result {
        assert!(
            !errors.errors.is_empty(),
            "Expected error for unclosed shortening"
        );
    }

    snapshot("word_parsing_tests__error_unclosed_shortening", &result);
}

/// Verifies cleaned text expands the shortening and strips parentheses.
#[test]
fn shortening_expanded_in_cleaned_text() {
    let result = parse_word("hel(lo)");

    if let Ok(word) = &result {
        assert_eq!(word.raw_text(), "hel(lo)");
        assert_eq!(
            word.cleaned_text(),
            "hello",
            "Shortening should be expanded in cleaned_text"
        );
        assert!(!word.cleaned_text().contains('('));
        assert!(!word.cleaned_text().contains(')'));
    }
}
