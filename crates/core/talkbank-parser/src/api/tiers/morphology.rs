//! Public entrypoint for parsing `%mor` tiers from CST nodes.
//!
//! This module re-exports the lower-level morphology parser used by
//! `parse_dependent_tier` and typed dispatch in the chat-file parser.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>

pub use crate::parser::tier_parsers::mor::parse_mor_tier;
