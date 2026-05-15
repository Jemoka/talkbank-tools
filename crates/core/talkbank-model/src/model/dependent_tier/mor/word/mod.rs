//! Word-level `%mor` item representation.
//!
//! This submodule isolates the `POS|lemma[-feature]*` token model so `%mor`
//! tier and item code can share one canonical lexical representation.
//! Keeping the type isolated also makes `%mor` token logic easier to unit-test
//! independently of tier-level alignment rules.
//! Constructors keep parsing permissive and defer strict lexical policy to
//! `%mor` validation so recovery paths can still produce useful diagnostics.
//!
//! Reference: <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>

#[path = "word.rs"]
mod word_item;

pub use word_item::MorWord;
