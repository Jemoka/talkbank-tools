//! Helper functions for dispatching based on tree-sitter node kinds
//!
//! These functions convert tree-sitter nodes to model types using structural dispatch
//! instead of text parsing.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Pauses>
//! - <https://talkbank.org/0info/manuals/CHAT.html#OverlapMarkers>

// ca and overlap modules removed — CA markers are now parsed by the direct parser
// (Phase 2 word coarsening), and overlap_point parsing lives in content/base/
mod pause;
mod separator;

pub(crate) use pause::parse_pause_node;
pub(crate) use separator::{parse_separator_like, parse_separator_node};
