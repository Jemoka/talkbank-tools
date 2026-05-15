//! Public entrypoints for `%pho` and `%mod` parsing from CST nodes.
//!
//! `%pho` and `%mod` share the same internal item model (`PhoTier`) and this
//! module re-exports both typed parsers used by dependent-tier dispatch.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Phonology_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Model_Phonology>

pub use crate::parser::tier_parsers::pho::{parse_mod_tier_from_unparsed, parse_pho_tier};
