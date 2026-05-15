//! Tests for quotation-following (`+"`) dialogue patterns.
//!
//! Covers both valid continuation sequences and ignored regression fixtures for
//! disabled `E341` validation paths.

use super::helpers::{check_cross_utterance_patterns, make_utterance};
use crate::Span;
use crate::model::{Linker, Terminator};

/// Pattern A quotation-following sequence validates cleanly.
///
/// A quoted terminator followed by `+"`-linked utterance(s) should emit no errors.
#[test]
fn test_quotation_follows_valid() {
    let utterances = vec![
        make_utterance(
            "CHI",
            vec!["the", "bear", "said"],
            vec![],
            Terminator::QuotedNewLine { span: Span::DUMMY },
        ),
        make_utterance(
            "CHI",
            vec!["please", "give", "me", "honey"],
            vec![Linker::QuotationFollows],
            Terminator::Period { span: Span::DUMMY },
        ),
    ];

    let errors = check_cross_utterance_patterns(&utterances);
    assert_eq!(errors.len(), 0, "Valid Pattern A should have no errors");
}

/// Multiple consecutive `+"` linked utterances remain valid.
///
/// This guards continuation cases where one quote-introducing utterance spans multiple follow-ups.
#[test]
fn test_quotation_follows_multiple_quotes() {
    let utterances = vec![
        make_utterance(
            "CHI",
            vec!["the", "bear", "said"],
            vec![],
            Terminator::QuotedNewLine { span: Span::DUMMY },
        ),
        make_utterance(
            "CHI",
            vec!["please", "give", "me", "honey"],
            vec![Linker::QuotationFollows],
            Terminator::Period { span: Span::DUMMY },
        ),
        make_utterance(
            "CHI",
            vec!["I'll", "carry", "you"],
            vec![Linker::QuotationFollows],
            Terminator::Period { span: Span::DUMMY },
        ),
    ];

    let errors = check_cross_utterance_patterns(&utterances);
    assert_eq!(
        errors.len(),
        0,
        "Multiple quoted utterances should be valid"
    );
}

/// Missing quotation-following utterance triggers `E341` (currently ignored).
///
/// Kept as a regression test fixture for when the rule is re-enabled.
#[test]
#[ignore = "E341 validation disabled (2025-12-28) - see cross_utterance/mod.rs for rationale"]
fn test_e341_quotation_follows_missing_quoted_utterance() {
    let utterances = vec![
        make_utterance(
            "CHI",
            vec!["the", "bear", "said"],
            vec![],
            Terminator::QuotedNewLine { span: Span::DUMMY },
        ),
        make_utterance(
            "MOT",
            vec!["what", "happened"],
            vec![],
            Terminator::Question { span: Span::DUMMY },
        ),
    ];

    let errors = check_cross_utterance_patterns(&utterances);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code.as_str(), "E341");
    assert!(
        errors[0]
            .message
            .contains("not followed by any subsequent utterance")
    );
}

/// Missing `+"` linker after a quoted terminator triggers `E341` (currently ignored).
///
/// This protects expected diagnostics once quotation-following validation is restored.
#[test]
#[ignore = "E341 validation disabled (2025-12-28) - see cross_utterance/mod.rs for rationale"]
fn test_e341_quotation_follows_no_linker() {
    let utterances = vec![
        make_utterance(
            "CHI",
            vec!["the", "bear", "said"],
            vec![],
            Terminator::QuotedNewLine { span: Span::DUMMY },
        ),
        make_utterance(
            "CHI",
            vec!["please", "give", "me", "honey"],
            vec![],
            Terminator::Period { span: Span::DUMMY },
        ),
    ];

    let errors = check_cross_utterance_patterns(&utterances);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code.as_str(), "E341");
    assert!(
        errors[0]
            .message
            .contains("not followed by quoted utterance")
    );
}

/// Mixed quotation patterns trigger `E341` (currently ignored).
///
/// The fixture documents ambiguous transitions between follows/precedes styles.
#[test]
#[ignore = "E341 validation disabled (2025-12-28) - see cross_utterance/mod.rs for rationale"]
fn test_e341_mixed_quotation_patterns() {
    let utterances = vec![
        make_utterance(
            "CHI",
            vec!["the", "bear", "said"],
            vec![],
            Terminator::QuotedNewLine { span: Span::DUMMY },
        ),
        make_utterance(
            "CHI",
            vec!["give", "me", "honey"],
            vec![Linker::QuotationFollows],
            Terminator::Period { span: Span::DUMMY },
        ),
        make_utterance(
            "CHI",
            vec!["I'll", "carry", "you"],
            vec![Linker::QuotationFollows],
            Terminator::QuotedPeriodSimple { span: Span::DUMMY },
        ),
    ];

    let errors = check_cross_utterance_patterns(&utterances);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code.as_str(), "E341");
    assert!(errors[0].message.contains("Mixed quotation patterns"));
}
