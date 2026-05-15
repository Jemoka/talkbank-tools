//! Supertype matcher for overlap-point marker node kinds.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#OverlapMarkers>

/// Check if a CST node kind string belongs to the `overlap_point_marker` supertype.
///
/// In the tree-sitter-talkbank grammar, `overlap_point_marker` is an abstract
/// supertype. Tree-sitter returns the concrete subtype name at runtime, so this
/// predicate tests whether a given `kind` is any of the concrete subtypes
/// (or the supertype name itself, for backwards compatibility).
///
/// # Parameters
///
/// - `kind`: The `node.kind()` string from a tree-sitter CST node.
///
/// # Returns
///
/// `true` if `kind` is `overlap_point_marker` or one of its concrete subtypes:
/// `bottom_overlap_begin_marker`, `bottom_overlap_end_marker`,
/// `top_overlap_begin_marker`, `top_overlap_end_marker`.
pub fn is_overlap_point_marker(kind: &str) -> bool {
    matches!(
        kind,
        "overlap_point_marker" |  // Keep for backwards compatibility
        "bottom_overlap_begin_marker" |
        "bottom_overlap_end_marker" |
        "top_overlap_begin_marker" |
        "top_overlap_end_marker"
    )
}
