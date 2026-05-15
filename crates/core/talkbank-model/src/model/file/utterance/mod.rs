//! Utterance model layer (main line + dependent tiers + validation metadata).
//!
//! In CHAT, each speaker turn is represented by one required main line and an
//! ordered set of optional dependent tiers.
//!
//! # CHAT Format Reference
//!
//! - [Main Tier Structure](https://talkbank.org/0info/manuals/CHAT.html#Main_Line)
//! - [Dependent Tiers Overview](https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers)
//!
//! # Example
//!
//! ```text
//! *CHI: I want cookie .
//! %mor: pro:sub|I v|want n|cookie .
//! %gra: 1|2|SUBJ 2|0|ROOT 3|2|OBJ 4|2|PUNCT
//! %com: reaches for cookie jar
//! ```

mod accessors;
mod core;
mod language_metadata_state;
mod parse_health;
mod utterance_language;
mod validate;

pub mod builder;
pub mod metadata;
pub mod serialization;
#[cfg(test)]
mod tests;

pub use core::Utterance;
pub use language_metadata_state::UtteranceLanguageMetadata;
pub use parse_health::{ParseHealth, ParseHealthState, ParseHealthTier};
pub use utterance_language::UtteranceLanguage;
