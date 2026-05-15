//! Test module for basic in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use crate::model::{FormType, WordLanguageMarker};

use super::helpers::{parse_word, snapshot};

/// Parses a plain lexical token without CHAT markers.
#[test]
fn simplest_word() {
    let result = parse_word("hello");
    snapshot("word_parsing_tests__simplest_word", &result);
}

/// A CA prosody arrow immediately before `@s` must not corrupt the
/// language-marker parse — `@s` here is `Shortcut`, not Explicit.
/// `@s` is a language marker, not a form-type, so `form_type` stays None.
#[test]
fn dona_with_ca_arrow_at_s_parses_as_shortcut_language_marker() {
    let result = parse_word("dona↑@s");

    let word = match &result {
        Ok(w) => w,
        Err(errors) => panic!("expected parse success for 'dona↑@s', got errors: {errors:?}"),
    };

    assert_eq!(word.lang, Some(WordLanguageMarker::Shortcut));
    assert_eq!(word.form_type, None);
}

/// Rejects empty input as an invalid word.
#[test]
fn error_empty_word() {
    let result = parse_word("");

    if let Err(errors) = &result {
        assert!(!errors.errors.is_empty(), "Expected at least 1 error");
    }

    snapshot("word_parsing_tests__error_empty_word", &result);
}

/// Parses a standard form-type marker (for example `@b`) and keeps cleaned text lexical.
#[test]
fn word_with_form_type() {
    let result = parse_word("hello@b");

    if let Ok(word) = &result {
        assert_eq!(word.raw_text(), "hello@b");
        assert_eq!(word.cleaned_text(), "hello");
        assert_eq!(word.form_type, Some(FormType::B));
    }

    snapshot("word_parsing_tests__word_with_form_type", &result);
}

/// Rejects unknown form-type markers.
#[test]
fn error_invalid_form_type() {
    let result = parse_word("hello@z");

    if let Err(errors) = &result {
        assert!(
            !errors.errors.is_empty(),
            "Expected error for invalid form type"
        );
    }

    snapshot("word_parsing_tests__error_invalid_form_type", &result);
}

/// Rejects dangling `@` markers with no form-type payload.
#[test]
fn error_missing_form_type() {
    let result = parse_word("hello@");

    if let Err(errors) = &result {
        assert!(
            !errors.errors.is_empty(),
            "Expected error for missing form type"
        );
    }

    snapshot("word_parsing_tests__error_missing_form_type", &result);
}

/// Parses user-defined form markers and preserves the custom label.
#[test]
fn user_defined_form_preserves_label() {
    let result = parse_word("is@z:foo");

    match &result {
        Ok(word) => {
            assert_eq!(
                word.form_type,
                Some(FormType::UserDefined("foo".to_string()))
            );
        }
        Err(errors) => {
            panic!("Expected parse success for @z:foo, got errors: {errors:?}");
        }
    }
}

/// Parses multi-language markers separated by `+` as explicit alternatives.
#[test]
fn word_with_multiple_language_marker_plus() {
    let result = parse_word("word@s:eng+spa");

    match &result {
        Ok(word) => {
            assert_eq!(word.raw_text(), "word@s:eng+spa");
            assert_eq!(word.cleaned_text(), "word");
            assert_eq!(
                word.lang,
                Some(WordLanguageMarker::multiple(vec![
                    "eng".into(),
                    "spa".into()
                ]))
            );
        }
        Err(errors) => {
            panic!("Expected parse success for @s:eng+spa, got errors: {errors:?}");
        }
    }
}

/// Parses ambiguous language markers separated by `&`.
#[test]
fn word_with_ambiguous_language_marker_ampersand() {
    let result = parse_word("word@s:eng&spa");

    match &result {
        Ok(word) => {
            assert_eq!(word.raw_text(), "word@s:eng&spa");
            assert_eq!(word.cleaned_text(), "word");
            assert_eq!(
                word.lang,
                Some(WordLanguageMarker::ambiguous(vec![
                    "eng".into(),
                    "spa".into()
                ]))
            );
        }
        Err(errors) => {
            panic!("Expected parse success for @s:eng&spa, got errors: {errors:?}");
        }
    }
}
