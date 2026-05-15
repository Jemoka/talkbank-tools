//! Public API for TalkBank parsing
//!
//! This module provides parsing APIs organized by level:
//! - `parser_api` — Fragment-aware parsing methods on `TreeSitterParser`
//! - `dependent_tier` — Dependent tier parsing (standalone, no parser handle needed)
//! - `tiers` — Granular tier parsing modules
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

pub mod dependent_tier;
mod parser_api;
mod parser_impl;
pub mod tiers;

// Re-export dependent tier parsing at module level
pub use dependent_tier::parse_dependent_tier;
pub use tiers::*;
