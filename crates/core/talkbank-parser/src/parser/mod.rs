//! Internal parser implementation modules.
//!
//! This layer owns CST traversal and conversion into `talkbank-model` types.
//! Callers that just need parsing APIs should use `crate::api` or top-level
//! crate entry points.
//!
//! # Module Organization
//!
//! - `chat_file_parser` — File-level parsing orchestration
//! - `tree_parsing` — CST-to-model conversion helpers
//! - `tier_parsers` — Tier-specific parsing implementations
//!
//! # Implementation notes
//!
//! - Prefer parser helper assertions before descending into child nodes.
//! - Report recoverable errors through sinks instead of dropping them.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

pub mod chat_file_parser;
pub mod participants;
pub mod tier_parsers;
pub mod tree_parsing;

/// Re-export the main parser type and initialization error.
pub use chat_file_parser::{ParserInitError, TreeSitterParser};
