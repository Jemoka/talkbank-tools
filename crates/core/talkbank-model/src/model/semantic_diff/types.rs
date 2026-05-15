//! Core types for semantic diff reporting.
//!
//! Semantic diffs compare parsed/serialized structures that represent CHAT files.
//! These types intentionally stay model-agnostic so derive-generated and manual
//! diff implementations can share one report vocabulary.
//!
//! Reference: <https://talkbank.org/0info/manuals/CHAT.html#File_Format>

use crate::Span;

/// Default maximum number of differences to collect before truncating.
///
/// This default keeps failure output readable while still surfacing enough
/// context to diagnose the first meaningful divergence.
pub const DEFAULT_MAX_DIFFS: usize = 20;

/// Kind of semantic difference between two values.
///
/// These categories are intentionally coarse and stable so callers can reason
/// about regressions without depending on model-specific internals.
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticDiffKind {
    /// Values at the same path differ.
    ValueMismatch,
    /// Collections differ in length.
    LengthMismatch,
    /// Key present on the left but absent on the right.
    MissingKey,
    /// Key present on the right but absent on the left.
    ExtraKey,
    /// Enum variants differ.
    VariantMismatch,
}

impl SemanticDiffKind {
    /// Returns the diff kind as a snake_case string.
    ///
    /// These stable string keys are used in reports and logs, so callers can
    /// rely on them for text-based diagnostics and snapshot tests.
    pub fn as_str(self) -> &'static str {
        match self {
            SemanticDiffKind::ValueMismatch => "value_mismatch",
            SemanticDiffKind::LengthMismatch => "length_mismatch",
            SemanticDiffKind::MissingKey => "missing_key",
            SemanticDiffKind::ExtraKey => "extra_key",
            SemanticDiffKind::VariantMismatch => "variant_mismatch",
        }
    }
}

/// A single semantic difference between two values.
///
/// Each record captures both structural location (`path`) and value-level
/// context (`left`/`right`), plus optional source span metadata.
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
#[derive(Debug, Clone)]
pub struct SemanticDifference {
    /// Dot-separated path to the differing field (e.g., `lines[0].utterance.main`).
    pub path: String,
    /// What kind of difference was found.
    pub kind: SemanticDiffKind,
    /// String representation of the left (original) value.
    pub left: String,
    /// String representation of the right (compared) value.
    pub right: String,
    /// Source span for error reporting.
    pub span: Option<Span>,
}
