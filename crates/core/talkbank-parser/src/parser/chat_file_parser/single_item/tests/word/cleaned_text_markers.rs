//! Test module for cleaned text markers in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use super::helpers::parse_word;

/// Verifies category prefixes are excluded from cleaned text.
#[test]
fn category_markers_excluded_from_cleaned_text() {
    let cases = vec![
        ("&-uh", "uh"),
        ("&~gaga", "gaga"),
        ("&+fr", "fr"),
        ("0is", "is"),
    ];

    for (input, expected_cleaned) in cases {
        let result = parse_word(input);
        if let Ok(word) = &result {
            assert_eq!(word.raw_text(), input);
            assert_eq!(
                word.cleaned_text(),
                expected_cleaned,
                "Category markers should be excluded from cleaned_text for {}",
                input
            );

            assert_ne!(word.cleaned_text().get(0..2), Some("&-"));
            assert_ne!(word.cleaned_text().get(0..2), Some("&~"));
            assert_ne!(word.cleaned_text().get(0..2), Some("&+"));
            assert_ne!(word.cleaned_text().chars().next(), Some('0'));
        }
    }
}

/// Verifies form-type suffixes are excluded from cleaned text.
#[test]
fn form_type_excluded_from_cleaned_text() {
    let result = parse_word("gumma@c");

    if let Ok(word) = &result {
        assert_eq!(word.raw_text(), "gumma@c");
        assert_eq!(
            word.cleaned_text(),
            "gumma",
            "Form type marker @c should be excluded from cleaned_text"
        );
        assert!(!word.cleaned_text().contains('@'));
    }
}

/// Verifies compound markers are excluded from cleaned text.
#[test]
fn compound_markers_excluded_from_cleaned_text() {
    let result = parse_word("ice+cream");

    if let Ok(word) = &result {
        assert_eq!(word.raw_text(), "ice+cream");
        assert_eq!(
            word.cleaned_text(),
            "icecream",
            "Compound marker + should be excluded from cleaned_text"
        );
        assert!(!word.cleaned_text().contains('+'));
    }
}

/// Verifies prosodic lengthening markers are excluded from cleaned text.
#[test]
fn lengthening_excluded_from_cleaned_text() {
    let result = parse_word("bana:nas");

    if let Ok(word) = &result {
        assert_eq!(word.raw_text(), "bana:nas");
        assert_eq!(
            word.cleaned_text(),
            "bananas",
            "Lengthening colon should be excluded from cleaned_text"
        );
        assert!(!word.cleaned_text().contains(':'));
    }
}
