//! Public entrypoints for action and gesture-adjacent dependent tiers.
//!
//! This module groups `%act`, `%cod`, and `%sin` parsers because they share
//! similar "aligned annotation on top of a main tier" parsing flow.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Action_Code>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Coding_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Gestures>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Speech_Act>

pub use crate::parser::tier_parsers::act::parse_act_tier;
pub use crate::parser::tier_parsers::cod::parse_cod_tier;
pub use crate::parser::tier_parsers::sin::parse_sin_tier;
