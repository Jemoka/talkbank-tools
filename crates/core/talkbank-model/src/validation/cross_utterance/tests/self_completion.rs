//! Tests for same-speaker self-completion (`+/`) sequences.
//!
//! Includes active happy-path coverage plus ignored fixtures for deferred
//! `E351/E352` rule enforcement.

use super::helpers::{check_cross_utterance_patterns, make_utterance};
use crate::Span;
use crate::model::{Linker, Terminator};

/// Valid self-completion sequence emits no cross-utterance errors.
///
/// The interrupted utterance and later `+/` continuation from same speaker should pair cleanly.
#[test]
fn test_self_completion_valid() {
    let utterances = vec![
        make_utterance(
            "CHI",
            vec!["so", "after", "the", "tower"],
            vec![],
            Terminator::Interruption { span: Span::DUMMY },
        ),
        make_utterance(
            "EXP",
            vec!["yeah"],
            vec![],
            Terminator::Period { span: Span::DUMMY },
        ),
        make_utterance(
            "CHI",
            vec!["I", "go", "straight", "ahead"],
            vec![Linker::SelfCompletion],
            Terminator::Period { span: Span::DUMMY },
        ),
    ];

    let errors = check_cross_utterance_patterns(&utterances);
    assert_eq!(
        errors.len(),
        0,
        "Valid self-completion should have no errors"
    );
}

/// Self-completion with no prior utterance triggers `E351`.
///
/// Activated by `--strict-linkers` flag (sets `enable_quotation_validation`).
#[test]
fn test_e351_self_completion_no_preceding_utterance() {
    let utterances = vec![make_utterance(
        "CHI",
        vec!["I", "go", "ahead"],
        vec![Linker::SelfCompletion],
        Terminator::Period { span: Span::DUMMY },
    )];

    let errors = check_cross_utterance_patterns(&utterances);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code.as_str(), "E351");
    assert!(
        errors[0]
            .message
            .contains("without any preceding utterance")
    );
}

/// Wrong terminator on source utterance triggers `E352`.
///
/// Self-completion source is expected to end with interruption marker `+/.`.
/// Activated by `--strict-linkers` flag.
///
/// The O(n) stack-based algorithm in `check_self_completion_all` tracks both
/// (a) a per-speaker stack of interruption indices (for O(1) `+/.` matching)
/// and (b) a per-speaker "last seen index" regardless of terminator. When a
/// `+,` has no matching `+/.` on the stack but the speaker has been seen
/// before, the algorithm emits E352 (wrong terminator). If the speaker has
/// never been seen, it emits E351 (no preceding utterance).
#[test]
fn test_e352_self_completion_wrong_terminator() {
    let utterances = vec![
        make_utterance(
            "CHI",
            vec!["so", "after", "the", "tower"],
            vec![],
            Terminator::Period { span: Span::DUMMY },
        ),
        make_utterance(
            "EXP",
            vec!["yeah"],
            vec![],
            Terminator::Period { span: Span::DUMMY },
        ),
        make_utterance(
            "CHI",
            vec!["I", "go", "ahead"],
            vec![Linker::SelfCompletion],
            Terminator::Period { span: Span::DUMMY },
        ),
    ];

    let errors = check_cross_utterance_patterns(&utterances);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code.as_str(), "E352");
    assert!(errors[0].message.contains("doesn't end with +/. "));
}
