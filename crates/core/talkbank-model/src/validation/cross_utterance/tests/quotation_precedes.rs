//! Tests for quotation-precedes patterns ending in quoted terminators.
//!
//! The suite documents accepted multi-linker sequences and ignored fixtures for
//! currently disabled `E344/E346` checks.

use super::helpers::{check_cross_utterance_patterns, make_utterance};
use crate::Span;
use crate::model::{Linker, Terminator};

/// Pattern B quotation-precedes sequence validates cleanly.
///
/// One or more `+"` linkers followed by a quoted terminator should be accepted.
#[test]
fn test_quotation_precedes_valid() {
    let utterances = vec![
        make_utterance(
            "CHI",
            vec!["please", "give", "me", "honey"],
            vec![Linker::QuotationFollows],
            Terminator::Period { span: Span::DUMMY },
        ),
        make_utterance(
            "CHI",
            vec!["the", "bear", "said"],
            vec![],
            Terminator::QuotedPeriodSimple { span: Span::DUMMY },
        ),
    ];

    let errors = check_cross_utterance_patterns(&utterances);
    assert_eq!(errors.len(), 0, "Valid Pattern B should have no errors");
}

/// Multiple queued quotation-precedes linkers remain valid.
///
/// This covers chained quoted content before the quoted terminator utterance.
#[test]
fn test_quotation_precedes_multiple_quotes() {
    let utterances = vec![
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
        make_utterance(
            "CHI",
            vec!["the", "bear", "said"],
            vec![],
            Terminator::QuotedPeriodSimple { span: Span::DUMMY },
        ),
    ];

    let errors = check_cross_utterance_patterns(&utterances);
    assert_eq!(
        errors.len(),
        0,
        "Multiple quoted utterances with Pattern B should be valid"
    );
}

/// Quoted terminator without preceding quote triggers `E344` (currently ignored).
///
/// Retained as regression coverage for when the validator rule is re-enabled.
#[test]
#[ignore = "E344 validation disabled (2025-12-28) - see cross_utterance/mod.rs for rationale"]
fn test_e344_quotation_precedes_without_preceding_quote() {
    let utterances = vec![make_utterance(
        "CHI",
        vec!["the", "bear", "said"],
        vec![],
        Terminator::QuotedPeriodSimple { span: Span::DUMMY },
    )];

    let errors = check_cross_utterance_patterns(&utterances);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code.as_str(), "E344");
    assert!(
        errors[0]
            .message
            .contains("without preceding quoted utterances")
    );
}

/// Missing quoted terminator after `+"` linkers triggers `E346` (currently ignored).
///
/// This fixture documents incomplete quotation-precedes sequences.
#[test]
#[ignore = "E346 validation disabled (2025-12-24) - see cross_utterance/mod.rs for rationale"]
fn test_e346_quoted_linker_missing_terminator() {
    let utterances = vec![
        make_utterance(
            "CHI",
            vec!["please", "give", "me", "honey"],
            vec![Linker::QuotationFollows],
            Terminator::Period { span: Span::DUMMY },
        ),
        make_utterance(
            "MOT",
            vec!["nice", "story"],
            vec![],
            Terminator::Period { span: Span::DUMMY },
        ),
    ];

    let errors = check_cross_utterance_patterns(&utterances);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code.as_str(), "E346");
    assert!(errors[0].message.contains("missing required terminator"));
}
