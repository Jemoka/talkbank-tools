//! Helper functions for tree-sitter parser
//!
//! This module contains utility functions extracted from the main parser
//! to keep individual files manageable:
//! - `error_checking` - Recursive error checking in parse trees
//! - `error_analysis` - Analysis and classification of ERROR nodes
//! - `node_dispatch` - Node kind dispatch helpers (separators, CA elements)
//! - `supertypes` - Supertype checking for grammar supertypes
//! - `cst_assertions` - CST structure validation (REQUIRED for robustness)
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

pub(crate) mod cst_assertions;
pub(crate) mod error_analysis;
pub(crate) mod error_checking;
pub(crate) mod node_dispatch;
pub(crate) mod supertypes;

// Re-export commonly used functions
#[allow(unused_imports)]
pub(crate) use cst_assertions::{
    assert_child_count_exact, assert_child_count_min, assert_child_kind, assert_child_kind_one_of,
    check_not_missing, expect_child, expect_child_at, extract_utf8_text,
};
pub(crate) use error_analysis::{
    analyze_dependent_tier_error, analyze_error_node, analyze_line_error,
};
pub(crate) use error_checking::check_for_errors_recursive;
pub(crate) use node_dispatch::{parse_pause_node, parse_separator_like, parse_separator_node};
// parse_ca_element_node, parse_ca_delimiter_node removed — word-internal CA markers
// are now parsed by the direct parser (Phase 2 word coarsening)
#[allow(unused_imports)]
pub(crate) use supertypes::{
    is_base_annotation, is_ca_delimiter, is_ca_element, is_dependent_tier, is_header, is_linker,
    is_overlap_point_marker, is_pre_begin_header, is_terminator,
};
