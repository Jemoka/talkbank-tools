//! Semantic diff trait for model types.
//!
//! This module complements `SemanticEq` by explaining *where* and *how* two
//! model values diverge. It underpins roundtrip diagnostics and migration
//! audits where boolean equality alone is not actionable.
//!
//! Provides structured, path-aware diffs to explain why `SemanticEq` fails.
//! Reference: <https://talkbank.org/0info/manuals/CHAT.html#File_Format>

mod context;
mod impls;
mod report;
mod source_utils;
mod tree_renderer;
mod types;

// Re-export path types (defined here to avoid circular deps)
pub use self::path::{PathSegment, SemanticPath};

// Re-export core types
pub use context::{SemanticDiffContext, normalize_span, normalize_span_option};
pub use report::SemanticDiffReport;
pub use tree_renderer::RenderMode;
pub use types::{DEFAULT_MAX_DIFFS, SemanticDiffKind, SemanticDifference};

/// Trait for computing semantic differences between values.
///
/// Unlike `PartialEq`, this trait provides structured diff information
/// including the path to the difference and the differing values.
pub trait SemanticDiff {
    /// Compute differences and add them to the report.
    fn semantic_diff_into(
        &self,
        other: &Self,
        path: &mut SemanticPath,
        report: &mut SemanticDiffReport,
        ctx: &mut SemanticDiffContext,
    );

    /// Convenience method to compute a full diff report.
    ///
    /// This allocates a fresh path/context/report trio and is ideal for
    /// one-shot comparisons in tests and diagnostics.
    fn semantic_diff(&self, other: &Self) -> SemanticDiffReport {
        let mut report = SemanticDiffReport::default();
        let mut path = SemanticPath::new();
        let mut ctx = SemanticDiffContext::new();
        self.semantic_diff_into(other, &mut path, &mut report, &mut ctx);
        report
    }
}

/// Path types for tracking location in the structure.
pub mod path {
    /// A path through a model structure, used to locate differences.
    ///
    /// Paths are built incrementally during recursive traversal and rendered in
    /// a stable field/index format for diagnostics and snapshots.
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
    #[derive(Debug, Clone, Default)]
    pub struct SemanticPath {
        segments: Vec<PathSegment>,
    }

    /// A single segment of a [`SemanticPath`].
    ///
    /// Segments are intentionally minimal to keep path formatting stable.
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
    #[derive(Debug, Clone)]
    pub enum PathSegment {
        /// Named struct field.
        Field(&'static str),
        /// Index into a sequence (Vec, slice).
        Index(usize),
    }

    impl SemanticPath {
        /// Creates a new empty [`SemanticPath`].
        ///
        /// Paths are built incrementally during recursive diff traversal and
        /// rendered only when emitting final diagnostics.
        pub fn new() -> Self {
            Self {
                segments: Vec::new(),
            }
        }

        /// Appends a named field segment to the path.
        ///
        /// Field segments become dot-delimited components in rendered paths.
        pub fn push_field(&mut self, field: &'static str) {
            self.segments.push(PathSegment::Field(field));
        }

        /// Appends an index segment to the path.
        ///
        /// Index segments render in bracket form, such as `[3]`.
        pub fn push_index(&mut self, index: usize) {
            self.segments.push(PathSegment::Index(index));
        }

        /// Removes the last segment from the path.
        ///
        /// Callers typically pair each push with a pop while unwinding recursion.
        pub fn pop(&mut self) {
            self.segments.pop();
        }

        /// Renders the path as dotted fields with bracketed indexes.
        fn render(&self) -> String {
            let mut out = String::new();
            for segment in &self.segments {
                match segment {
                    PathSegment::Field(name) => {
                        if !out.is_empty() {
                            out.push('.');
                        }
                        out.push_str(name);
                    }
                    PathSegment::Index(index) => {
                        out.push('[');
                        out.push_str(&index.to_string());
                        out.push(']');
                    }
                }
            }

            if out.is_empty() { "/".to_string() } else { out }
        }
    }

    impl std::fmt::Display for SemanticPath {
        /// Formats the path for diagnostics.
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(&self.render())
        }
    }
}
