//! Public entrypoint for parsing `%gra` grammatical-relation tiers.
//!
//! This module exposes the typed parser used by dependent-tier dispatch.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Grammatical_Relations>
//! - <https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier>

pub use crate::parser::tier_parsers::gra::parse_gra_tier;
