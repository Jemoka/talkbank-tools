//! Graph-subsystem-specific errors.
//!
//! Scoping these variants inside the graph module (rather than top-
//! level on [`LspBackendError`]) keeps the crate-wide error enum
//! narrow — only graph tests need to match on graph-specific failure
//! modes. The `?` operator lifts `GraphEdgeError` to
//! `LspBackendError::Graph` via `#[from]` at the subsystem boundary.

use thiserror::Error;

/// Failures produced when building dependency-graph edges from a
/// `%gra` tier plus its alignment metadata.
///
/// These are not user-editable errors — they indicate that the
/// `AlignmentSet` is out of sync with the tiers, typically because
/// a document edit invalidated the cached alignment. Emitted only
/// by [`append_dependency_edges`](super::edges::append_dependency_edges).
#[derive(Debug, Error)]
pub enum GraphEdgeError {
    /// A `%gra` alignment pair refers to a `%mor` chunk index that is
    /// outside the chunk sequence — a stale alignment, not a
    /// user-editable error.
    #[error("Alignment references invalid chunk index {chunk_index}")]
    InvalidChunkIndex {
        /// The out-of-range chunk index.
        chunk_index: usize,
    },

    /// A `%gra` relation has no corresponding alignment pair — fires
    /// when `gra_alignment.pairs` is out of sync with
    /// `gra_tier.relations.len()`.
    #[error("No alignment pair found for %gra relation at index {rel_index}")]
    MissingAlignmentPair {
        /// The `%gra` relation position (0-indexed into `gra_tier.relations`).
        rel_index: usize,
    },

    /// A `%gra` relation's `head` field points to a word position that
    /// has no corresponding `%mor` chunk.
    #[error("Invalid %gra head index {head}: no chunk at position {chunk_index}")]
    InvalidHeadIndex {
        /// The raw 1-indexed `head` value as written in the `%gra` tier.
        head: usize,
        /// The computed 0-indexed chunk position that turned out to be
        /// out of range.
        chunk_index: usize,
    },
}
