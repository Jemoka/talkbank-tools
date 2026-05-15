//! File-layer transcript model (`ChatFile` / `Line` / `Utterance`).
//!
//! This module defines the in-memory structure used after parsing and before
//! serialization/validation:
//! - `ChatFile`: full transcript with interleaved lines and participant metadata
//! - `Line`: one file-order unit (`@header` or utterance)
//! - `Utterance`: main tier plus dependent tiers and runtime metadata
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Line>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

mod chat_file;
mod line;
mod utterance;

pub use chat_file::{ChatFile, ChatFileLines};
pub use line::Line;
pub use utterance::{
    ParseHealth, ParseHealthState, ParseHealthTier, Utterance, UtteranceLanguage,
    UtteranceLanguageMetadata,
};
