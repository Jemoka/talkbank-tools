#![allow(dead_code)]
//! Supertype checking for tree-sitter grammar.
//!
//! Tree-sitter supertypes are abstract node categories that group related concrete types.
//! When a supertype is defined, tree-sitter returns the concrete type name, not the
//! supertype name. These helpers check if a node kind is one of the concrete subtypes.
//!
//! Generated from tree-sitter-talkbank grammar supertypes.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Terminators>

mod annotations;
mod ca;
mod headers;
mod linkers;
mod overlap;
mod terminators;
mod tiers;

pub use annotations::is_base_annotation;
pub use ca::{is_ca_delimiter, is_ca_element};
pub use headers::{is_header, is_pre_begin_header};
pub use linkers::is_linker;
pub use overlap::is_overlap_point_marker;
pub use terminators::is_terminator;
pub use tiers::is_dependent_tier;

#[cfg(test)]
mod tests;
