//! Parsing for regular bracketed groups in main-tier content.
//!
//! The group hierarchy described in the manual (Scoped Symbols + Main Tier sections) uses `<...>` blocks
//! with optional nested content and annotations. This module exposes the helpers needed to walk the CST
//! nodes, convert `UtteranceContent` into `BracketedItem`, and emit typed `Group` or `AnnotatedGroup` values.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>
//! - <https://talkbank.org/0info/manuals/CHAT.html#OverlapMarkers>

mod contents;
mod nested;
mod parser;

pub(crate) use contents::convert_to_group_content;
pub(crate) use nested::parse_nested_content;
pub(crate) use parser::parse_group_content;
