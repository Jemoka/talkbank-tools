//! Tier-specific dependent-tier parsers.
//!
//! Each submodule accepts CST nodes already identified as a specific tier and
//! converts them into typed model values while streaming parse errors.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Phonology_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Comment_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Coding_Tier>

pub mod act;
pub mod cod;
pub mod dependent_tier;
pub mod gra;
pub mod mor;
pub mod pho;
pub mod sin;
pub mod text;
pub mod wor;

// Re-export public parsing functions for convenience within parser module
pub use dependent_tier::parse_dependent_tier;
