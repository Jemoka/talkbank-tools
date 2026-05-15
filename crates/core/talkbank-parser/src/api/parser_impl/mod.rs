//! Implementation helpers for `TreeSitterParser` fragment methods.
//!
//! The fragment methods in `api/parser_api.rs` delegate to helpers in this
//! submodule so wrapper parsing, span adjustment, and error multiplexing stay in
//! one place.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

mod helpers;

// Re-export helpers for use in parser_api
pub(super) use helpers::{wrapper_parse_generic_tier, wrapper_parse_tier};
