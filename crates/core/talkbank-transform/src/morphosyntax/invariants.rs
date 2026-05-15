//! Grammatical-invariant rewrites over typed UD sentences.
//!
//! This module corrects specific classes of Stanza misanalysis where the entire
//! UD output violates a universal grammatical constraint.

mod finite_verb_main_clause;

use crate::morphosyntax::{MappingContext, UdSentence, lang2};

pub use finite_verb_main_clause::rescue_english_copula_progressive;

/// Apply all language-appropriate grammatical-invariant rewrites to a UD
/// sentence.
pub fn apply_grammatical_invariants(sentence: &UdSentence, ctx: &MappingContext) -> UdSentence {
    match lang2(ctx.lang.as_str()) {
        "en" => finite_verb_main_clause::rescue_english_copula_progressive(sentence),
        _ => sentence.clone(),
    }
}
