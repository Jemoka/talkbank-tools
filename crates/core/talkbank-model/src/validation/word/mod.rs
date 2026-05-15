//! Word-level validation entrypoints and submodules.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Word_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#SecondLanguage_Marker>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Part_of_Speech>
//!
//! `structure` covers token-internal form constraints, while `language` handles
//! language-marker resolution and language-dependent policies (such as digit
//! allowance checks).

pub mod language;
pub mod structure;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod snapshot_tests;

pub use language::{LanguageResolutionOutcome, resolve_word_language};
