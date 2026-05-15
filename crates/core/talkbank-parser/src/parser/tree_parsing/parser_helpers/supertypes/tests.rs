//! Tests for this subsystem.
//!

use super::{is_base_annotation, is_dependent_tier, is_header, is_terminator};

/// Tests is base annotation.
#[test]
fn test_is_base_annotation() {
    // Leaf annotations (Phase 5 coarsening inlined all intermediate wrappers)
    assert!(is_base_annotation("error_marker_annotation"));
    assert!(is_base_annotation("explanation_annotation"));
    assert!(is_base_annotation("indexed_overlap_precedes"));
    assert!(is_base_annotation("retrace_complete"));
    assert!(is_base_annotation("scoped_stressing"));
    assert!(is_base_annotation("exclude_marker"));
    // Supertype wrapper
    assert!(is_base_annotation("base_annotation"));
    // Not annotations
    assert!(!is_base_annotation("word"));
    assert!(!is_base_annotation("header"));
    // Removed intermediate wrappers (no longer in grammar)
    assert!(!is_base_annotation("retrace_marker"));
    assert!(!is_base_annotation("overlap"));
    assert!(!is_base_annotation("scoped_symbol"));
}

/// Tests is terminator.
#[test]
fn test_is_terminator() {
    assert!(is_terminator("period"));
    assert!(is_terminator("question"));
    assert!(is_terminator("interruption"));
    assert!(is_terminator("terminator"));
    assert!(!is_terminator("word"));
}

/// Tests is header.
#[test]
fn test_is_header() {
    assert!(is_header("languages_header"));
    assert!(is_header("participants_header"));
    assert!(is_header("id_header"));
    assert!(is_header("header"));
    assert!(!is_header("utterance"));
}

/// Tests is dependent tier.
#[test]
fn test_is_dependent_tier() {
    assert!(is_dependent_tier("mor_dependent_tier"));
    assert!(is_dependent_tier("gra_dependent_tier"));
    assert!(is_dependent_tier("pho_dependent_tier"));
    assert!(is_dependent_tier("dependent_tier"));
    assert!(!is_dependent_tier("main_tier"));
}
