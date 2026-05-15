//! Test module for categories in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use crate::model::WordCategory;

use super::helpers::{parse_word, snapshot};

/// Parses filler-prefixed words into `WordCategory::Filler`.
#[test]
fn word_with_filler_category() {
    let result = parse_word("&-uh");

    if let Ok(word) = &result {
        assert_eq!(word.raw_text(), "&-uh");
        assert_eq!(word.cleaned_text(), "uh");
        assert_eq!(word.category, Some(WordCategory::Filler));
    }

    snapshot("word_parsing_tests__word_with_filler_category", &result);
}

/// Parses nonword-prefixed words into `WordCategory::Nonword`.
#[test]
fn word_with_nonword_category() {
    let result = parse_word("&~gaga");

    if let Ok(word) = &result {
        assert_eq!(word.raw_text(), "&~gaga");
        assert_eq!(word.cleaned_text(), "gaga");
        assert_eq!(word.category, Some(WordCategory::Nonword));
    }

    snapshot("word_parsing_tests__word_with_nonword_category", &result);
}

/// Parses omission-prefixed words into `WordCategory::Omission`.
#[test]
fn word_with_omission_category() {
    let result = parse_word("0is");

    if let Ok(word) = &result {
        assert_eq!(word.raw_text(), "0is");
        assert_eq!(word.cleaned_text(), "is");
        assert_eq!(word.category, Some(WordCategory::Omission));
    }

    snapshot("word_parsing_tests__word_with_omission_category", &result);
}

/// Parses fragment-prefixed words into `WordCategory::PhonologicalFragment`.
#[test]
fn word_with_phonological_fragment_category() {
    let result = parse_word("&+fr");

    if let Ok(word) = &result {
        assert_eq!(word.raw_text(), "&+fr");
        assert_eq!(word.cleaned_text(), "fr");
        assert_eq!(word.category, Some(WordCategory::PhonologicalFragment));
    }

    snapshot(
        "word_parsing_tests__word_with_phonological_fragment_category",
        &result,
    );
}
