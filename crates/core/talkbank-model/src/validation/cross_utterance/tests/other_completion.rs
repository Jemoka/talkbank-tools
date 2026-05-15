//! Tests for `+//` other-completion cross-utterance patterns.
//!
//! Includes both currently-active validations and ignored TDD fixtures for
//! rules that are intentionally deferred.

use super::helpers::{check_cross_utterance_patterns, make_utterance};
use crate::Span;
use crate::model::{Linker, Terminator};

/// Valid other-completion sequence emits no errors.
///
/// A trailing-off utterance from one speaker followed by `+//` from another should pair cleanly.
#[test]
fn test_other_completion_valid() {
    let utterances = vec![
        make_utterance(
            "HEL",
            vec!["if", "Bill", "had", "known"],
            vec![],
            Terminator::TrailingOff { span: Span::DUMMY },
        ),
        make_utterance(
            "WIN",
            vec!["he", "would", "have", "come"],
            vec![Linker::OtherCompletion],
            Terminator::Period { span: Span::DUMMY },
        ),
    ];

    let errors = check_cross_utterance_patterns(&utterances);
    assert_eq!(
        errors.len(),
        0,
        "Valid other-completion should have no errors"
    );
}

/// Other-completion without a preceding utterance triggers `E353`.
///
/// Activated by `--strict-linkers` flag (sets `enable_quotation_validation`).
#[test]
fn test_e353_other_completion_no_preceding() {
    let utterances = vec![make_utterance(
        "WIN",
        vec!["he", "would", "have", "come"],
        vec![Linker::OtherCompletion],
        Terminator::Period { span: Span::DUMMY },
    )];

    let errors = check_cross_utterance_patterns(&utterances);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code.as_str(), "E353");
}

/// Wrong source terminator for other-completion triggers `E354`.
///
/// The source utterance should end with trailing-off marker `+...`.
/// Activated by `--strict-linkers` flag.
#[test]
fn test_e354_other_completion_wrong_terminator() {
    let utterances = vec![
        make_utterance(
            "HEL",
            vec!["if", "Bill", "had", "known"],
            vec![],
            Terminator::Period { span: Span::DUMMY },
        ),
        make_utterance(
            "WIN",
            vec!["he", "would", "have", "come"],
            vec![Linker::OtherCompletion],
            Terminator::Period { span: Span::DUMMY },
        ),
    ];

    let errors = check_cross_utterance_patterns(&utterances);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code.as_str(), "E354");
    assert!(errors[0].message.contains("doesn't end with +..."));
}

/// Same-speaker other-completion triggers `E355`.
///
/// Other-completion requires speaker change between source and completion utterances.
/// Activated by `--strict-linkers` flag.
#[test]
fn test_e355_other_completion_same_speaker() {
    let utterances = vec![
        make_utterance(
            "CHI",
            vec!["if", "Bill", "had", "known"],
            vec![],
            Terminator::TrailingOff { span: Span::DUMMY },
        ),
        make_utterance(
            "CHI",
            vec!["he", "would", "have", "come"],
            vec![Linker::OtherCompletion],
            Terminator::Period { span: Span::DUMMY },
        ),
    ];

    let errors = check_cross_utterance_patterns(&utterances);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code.as_str(), "E355");
    assert!(errors[0].message.contains("same speaker"));
}
