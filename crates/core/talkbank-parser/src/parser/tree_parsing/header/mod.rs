//! Header parsing using tree-sitter nodes
//!
//! This module provides functions to extract structured header data from tree-sitter nodes.
//! **CRITICAL**: NO string parsing! All data is extracted from tree-sitter child nodes.
//!
//! # Philosophy
//!
//! - Extract data from tree-sitter nodes by position/field name
//! - Never parse strings - tree-sitter has already done the parsing
//! - Return typed `Header` variants for valid structures
//! - Return `Header::Unknown` (with parse_reason) when required CST parts are missing
//! - Use error recovery - stream errors and preserve as much signal as possible
//!
//! # Module Structure
//!
//! - `id/` - @ID header parsing (~260 lines)
//! - `participants.rs` - @Participants header parsing (~170 lines)
//! - `metadata/` - @Languages, @PID, @Media, @Situation, @Types, @T
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#ID_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Languages_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Media_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Situation_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Types_Header>

mod id;
mod metadata;
mod participants;

// Re-export all header parsing functions
pub use id::parse_id_header;
pub use metadata::{
    parse_languages_header, parse_media_header, parse_pid_header, parse_situation_header,
    parse_t_header, parse_types_header,
};
pub use participants::parse_participants_header;
