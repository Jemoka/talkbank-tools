//! Tree-sitter parsing implementation details
//!
//! Converts tree-sitter concrete syntax tree (CST) nodes into talkbank-model types.
//!
//! # Implementation Constraints
//!
//! - Use node type constants from `crate::node_types`
//! - Do not parse by string slicing; rely on CST nodes
//! - Check for missing nodes and report errors via `ErrorSink`
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Words>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Terminators>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

pub mod bullet_content;
pub mod dependent_tier;
pub mod freecode;
pub mod header;
pub mod helpers;
pub mod main_tier;
// Some items (BULLET_CHAR, parse_bullet_text, parse_media_bullet) are not
// currently called outside tests but remain for future coarsened-grammar
// token parsing paths.
#[allow(dead_code)]
pub mod media_bullet;
pub mod node_types {}
pub mod parser_helpers;
pub mod postcode;
