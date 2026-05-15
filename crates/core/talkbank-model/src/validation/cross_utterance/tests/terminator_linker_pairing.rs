//! Focused tests for linker/terminator pairing integrity.
//!
//! These fixtures keep minimal malformed sequences around so disabled pairing
//! rules can be re-enabled with explicit regression coverage.

use super::helpers::{check_cross_utterance_patterns, make_utterance};
use crate::Span;
use crate::model::{Linker, Terminator};

/// Quoted terminator without prior quotation linker triggers `E344` (currently ignored).
///
/// This captures the minimal broken quotation-precedes pattern.
#[test]
#[ignore = "E344 validation disabled (2025-12-28) - see cross_utterance/mod.rs for rationale"]
fn test_quoted_terminator_no_linker() {
    // +". terminator without preceding +" linker from same speaker
    let utterances = vec![make_utterance(
        "CHI",
        vec!["the", "bear", "said"],
        vec![],
        Terminator::QuotedPeriodSimple { span: Span::DUMMY },
    )];

    let errors = check_cross_utterance_patterns(&utterances);
    assert_eq!(errors.len(), 1);
    // E344: Quotation precedes terminator without preceding quoted utterances
    assert_eq!(errors[0].code.as_str(), "E344");
}

/// Lone quotation linker with non-quoted terminator triggers `E346` (currently ignored).
///
/// The fixture documents an incomplete quotation-linker sequence.
#[test]
#[ignore = "E346 validation disabled (2025-12-24) - see cross_utterance/mod.rs for rationale"]
fn test_quoted_linker_wrong_terminator() {
    // +" linker should have corresponding quoted utterances or terminators
    // A single utterance with +" but regular terminator indicates incomplete pattern
    let utterances = vec![make_utterance(
        "CHI",
        vec!["hello", "there"],
        vec![Linker::QuotationFollows],
        Terminator::Period { span: Span::DUMMY },
    )];

    let errors = check_cross_utterance_patterns(&utterances);
    // This pattern is caught as incomplete quotation pattern
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code.as_str(), "E346");
}
