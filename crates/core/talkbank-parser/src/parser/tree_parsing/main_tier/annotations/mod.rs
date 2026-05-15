//! Annotation parsing for main tier content
//!
//! This module handles parsing of:
//! - Replacements [: word1 word2...] and [:: word1 word2...]
//! - Scoped annotations [*], [//], [= text], etc.
//! - Overlap markers [<], [>]
//! - Retrace markers [//], [///], [/]
//! - Error markers [*], [* code]
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Retracing_and_Repetition>
//! - <https://talkbank.org/0info/manuals/CHAT.html#OverlapMarkers>

mod error_annotation;
mod helpers;
mod overlap;
mod replacement;
mod retrace;
mod scoped;

// Re-export public API
pub(crate) use replacement::parse_replacement;
pub(crate) use scoped::parse_scoped_annotations;

// Internal functions used by scoped.rs
