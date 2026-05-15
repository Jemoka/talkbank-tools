//! Public entrypoints for free-text dependent tiers.
//!
//! The re-exported parsers cover `%com`, `%exp`, `%add`, `%spa`, `%sit`,
//! `%gpx`, and `%int`, each of which is represented as text/bullet content
//! rather than POS/dependency-like structural tuples.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Comment_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Explanation_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Addressee_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Situation_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Intonation_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Speech_Act>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Gems>

pub use crate::parser::tier_parsers::text::{
    parse_add_tier, parse_com_tier, parse_exp_tier, parse_gpx_tier, parse_int_tier, parse_sit_tier,
    parse_spa_tier,
};
