//! Shared test helpers for cross-utterance validation suites.
//!
//! Helper functions keep fixture construction compact and make individual tests
//! emphasize dialogue sequencing over boilerplate model assembly.

use crate::ParseError;
use crate::model::{Linker, MainTier, Terminator, Utterance, UtteranceContent, Word};
use crate::validation::ValidationContext;

/// Executes cross-utterance validation with quotation/linker validation enabled.
///
/// Test fixtures use this to focus on utterance sequencing rather than context
/// construction boilerplate. Quotation validation is enabled so that E341-E355
/// checks fire during tests.
pub fn check_cross_utterance_patterns(utterances: &[Utterance]) -> Vec<ParseError> {
    let context = ValidationContext::default().with_quotation_validation(true);
    crate::validation::cross_utterance::check_cross_utterance_patterns(utterances, &context)
}

/// Builds a minimal utterance fixture for cross-utterance tests.
///
/// The helper wires words, linkers, and terminator into a `MainTier` so tests
/// can describe dialogue sequences compactly.
pub fn make_utterance(
    speaker: &str,
    words: Vec<&str>,
    linkers: Vec<Linker>,
    terminator: Terminator,
) -> Utterance {
    let content: Vec<UtteranceContent> = words
        .into_iter()
        .map(|w| UtteranceContent::Word(Box::new(Word::new_unchecked(w, w))))
        .collect();

    let main = MainTier::new(speaker.to_string(), content, Some(terminator)).with_linkers(linkers);
    Utterance::new(main)
}
