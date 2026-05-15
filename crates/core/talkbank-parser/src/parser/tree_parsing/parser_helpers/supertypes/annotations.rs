//! Supertype matcher for scoped/base annotation node kinds.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Retracing_and_Repetition>

/// Check if a node kind is a `base_annotation` subtype
///
/// After Phase 5 coarsening, all 18 leaf alternatives are listed directly
/// (no intermediate wrappers like `overlap`, `scoped_symbol`, `retrace_marker`).
pub fn is_base_annotation(kind: &str) -> bool {
    matches!(
        kind,
        "base_annotation" |  // Keep for backwards compatibility (supertype wrapper)
        "indexed_overlap_precedes" |
        "indexed_overlap_follows" |
        "scoped_stressing" |
        "scoped_contrastive_stressing" |
        "scoped_best_guess" |
        "scoped_uncertain" |
        "explanation_annotation" |
        "para_annotation" |
        "alt_annotation" |
        "percent_annotation" |
        "duration_annotation" |
        "error_marker_annotation" |
        "retrace_complete" |
        "retrace_partial" |
        "retrace_multiple" |
        "retrace_reformulation" |
        "retrace_uncertain" |
        "exclude_marker"
    )
}
