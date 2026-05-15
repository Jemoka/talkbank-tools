//! File-level CHAT parsing orchestration for the tree-sitter backend.
//!
//! This module coordinates CST traversal for complete CHAT documents and
//! delegates each line to header/utterance/dependent-tier subparsers.
//!
//! # Module Organization
//!
//! - `parser_struct.rs` - TreeSitterParser struct and new/default impls
//! - `chat_file/` - parse_chat_file implementation
//! - `header_parser.rs` - Header node parsing
//! - `utterance_parser.rs` - Utterance node parsing
//! - `dependent_tier_dispatch/` - dependent tier routing logic
//! - `single_item/` - parse_utterance, parse_main_tier, parse_word
//! - `header_dispatch/` - parse_header and header node finding
//!
//! # Parser Orchestration Rules
//!
//! - Verify CST structure before parsing children
//! - Stream errors via `ErrorSink`; avoid fail-fast parsing
//! - Use a two-stage strategy:
//!   1. Cheap dispatch on tier/header prefixes
//!   2. CST-driven parsing for the selected path
//! - Recovery must not inject dummy/sentinel model payloads
//! - On malformed tiers, emit diagnostics and taint parse-health domains for downstream gating
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

pub(crate) mod chat_file;
mod dependent_tier_dispatch;
mod header_dispatch;
mod header_parser;
mod parser_struct;
mod single_item;
mod utterance_parser;

// Re-export the main parser type
pub use parser_struct::ParserInitError;
pub use parser_struct::TreeSitterParser;

// Re-export minimal CHAT constants for use in ChatParser trait impl
pub(crate) use single_item::helpers::{MINIMAL_CHAT_PREFIX, MINIMAL_CHAT_SUFFIX};
