//! Context-sensitive analysis for tree-sitter `ERROR` nodes.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

mod dependent_tier;
mod file;
mod header;
mod line;
mod utterance;

pub(crate) use dependent_tier::{
    analyze_dependent_tier_error, analyze_dependent_tier_error_with_context,
};
pub(crate) use file::analyze_error_node;
pub(crate) use line::analyze_line_error;
