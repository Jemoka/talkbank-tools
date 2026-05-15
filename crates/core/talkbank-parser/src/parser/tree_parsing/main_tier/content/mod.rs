//! Main tier content parsing
//!
//! This module handles parsing utterance content including:
//! - Words, nonwords (events, zero/action, other spoken events), pauses
//! - Groups with annotations
//! - Base content types
//!
//! The content parsing is organized into focused modules:
//! - `errors` - Error analysis for word/content parsing
//! - `nonword` - Nonword content parsing (events, zero/action, other spoken events)
//! - `word` - Word content with optional annotations and replacements
//! - `pho_group` - Phonological group parsing (`<word word> [*]`)
//! - `sin_group` - Sign/gesture group parsing
//! - `quotation` - Quotation parsing (+"/)
//! - `base` - Base content types (pauses, freecodes)
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Words>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Pauses>
//! - <https://talkbank.org/0info/manuals/CHAT.html#OverlapMarkers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Freecodes>

mod base;
mod errors;
mod group;
mod nonword;
mod pho_group;
mod quotation;
mod sin_group;
mod word;

// Re-export helper functions
pub(crate) use errors::analyze_word_error;

// Re-export overlap_point parser for use in structure parsing
pub(crate) use base::parse_overlap_point;

// Re-export content parsers for use in structure/contents.rs
pub(crate) use base::parse_base_content;
pub(crate) use group::parse_group_content;
pub(crate) use pho_group::parse_pho_group_content;
pub(crate) use quotation::parse_quotation_content;
pub(crate) use sin_group::parse_sin_group_content;
