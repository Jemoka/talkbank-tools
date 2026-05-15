//! Cross-utterance edge-case fixtures.
//!
//! Focuses on mixed-speaker and multi-rule interaction scenarios that are easy
//! to miss when testing only canonical linker patterns.

use super::helpers::{check_cross_utterance_patterns, make_utterance};
use crate::Span;
use crate::model::{Linker, Terminator};

/// Quotation linkage tolerates intervening utterances from other speakers.
///
/// This protects real transcripts where acknowledgement turns appear between quote setup and follow-up.
#[test]
fn test_quotation_with_intervening_speakers() {
    let utterances = vec![
        make_utterance(
            "CHI",
            vec!["the", "bear", "said"],
            vec![],
            Terminator::QuotedNewLine { span: Span::DUMMY },
        ),
        make_utterance(
            "MOT",
            vec!["uh", "huh"],
            vec![],
            Terminator::Period { span: Span::DUMMY },
        ),
        make_utterance(
            "CHI",
            vec!["please", "give", "me", "honey"],
            vec![Linker::QuotationFollows],
            Terminator::Period { span: Span::DUMMY },
        ),
    ];

    let errors = check_cross_utterance_patterns(&utterances);
    assert_eq!(
        errors.len(),
        0,
        "Quotation with intervening speakers should be valid"
    );
}

/// Mixed-invalid pattern emits the expected active validation code(s).
///
/// With some rules disabled, this currently asserts only the still-enabled `E341` path.
#[test]
#[ignore = "E341 validation disabled (2025-12-28) - see cross_utterance/mod.rs for rationale"]
fn test_multiple_validation_errors() {
    let utterances = vec![
        make_utterance(
            "CHI",
            vec!["she", "said"],
            vec![],
            Terminator::QuotedNewLine { span: Span::DUMMY },
        ),
        make_utterance(
            "MOT",
            vec!["okay"],
            vec![],
            Terminator::Period { span: Span::DUMMY },
        ),
        make_utterance(
            "EXP",
            vec!["continue"],
            vec![Linker::SelfCompletion],
            Terminator::Period { span: Span::DUMMY },
        ),
    ];

    let errors = check_cross_utterance_patterns(&utterances);
    // NOTE: E351 (self-completion) validation is disabled as of 2025-12-24
    // Only E341 (quotation follows) is validated
    assert_eq!(errors.len(), 1, "Should detect E341 (quotation follows)");

    let codes: Vec<&str> = errors.iter().map(|e| e.code.as_str()).collect();
    assert!(codes.contains(&"E341"), "Should detect E341");
}
