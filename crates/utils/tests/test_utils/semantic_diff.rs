//! Shared semantic diff wrapper for roundtrip tests.

use talkbank_model::model::{
    ChatFile, SemanticDiff, SemanticDiffContext, SemanticDiffReport, SemanticPath,
};

/// Analyze semantic differences, stopping at first mismatch.
///
/// We fail fast because after the first diff, subsequent items are likely
/// off-by-one and would produce meaningless cascading errors.
pub fn analyze_semantic_diff(left: &ChatFile, right: &ChatFile) -> SemanticDiffReport {
    // Fail fast: only report first difference
    let mut report = SemanticDiffReport::new(1);
    let mut path = SemanticPath::new();
    let mut ctx = SemanticDiffContext::new();
    left.semantic_diff_into(right, &mut path, &mut report, &mut ctx);
    report
}
