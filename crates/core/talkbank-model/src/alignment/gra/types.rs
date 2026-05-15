//! Data structures for `%mor` ↔ `%gra` alignment results.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier>

use crate::ParseError;
use crate::alignment::indices::{GraIndex, MorChunkIndex};
use schemars::JsonSchema;
use talkbank_derive::SpanShift;

/// Result of aligning `%mor` chunks to `%gra` relations.
///
/// `pairs` keeps positional correspondence (including placeholders for
/// mismatches), while `errors` carries diagnostics explaining divergence.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, JsonSchema, SpanShift)]
pub struct GraAlignment {
    /// Alignment pairs (mor_chunk_index, gra_relation_index)
    ///
    /// Indices are positions in the chunk sequence.
    /// `None` in either position indicates a placeholder due to misalignment.
    pub pairs: Vec<GraAlignmentPair>,

    /// Diagnostics emitted while enforcing `%mor`/`%gra` alignment invariants.
    pub errors: Vec<ParseError>,
}

impl GraAlignment {
    /// Creates an empty alignment accumulator.
    ///
    /// Used by alignment passes before pairs and diagnostics are appended.
    pub fn new() -> Self {
        Self {
            pairs: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// Appends one `%mor`/`%gra` alignment row.
    ///
    /// Returns `Self` for builder-style use inside alignment loops.
    pub fn with_pair(mut self, pair: GraAlignmentPair) -> Self {
        self.pairs.push(pair);
        self
    }

    /// Appends one alignment diagnostic.
    ///
    /// Callers may accumulate multiple diagnostics when continuing after mismatch.
    pub fn with_error(mut self, error: ParseError) -> Self {
        self.errors.push(error);
        self
    }

    /// Returns `true` when no alignment diagnostics were emitted.
    ///
    /// A `true` value implies every row in `pairs` is a complete one-to-one mapping.
    pub fn is_error_free(&self) -> bool {
        self.errors.is_empty()
    }
}

impl Default for GraAlignment {
    /// Builds an empty `%mor`/`%gra` alignment result.
    fn default() -> Self {
        Self::new()
    }
}

/// One positional mapping row between a `%mor` chunk and a `%gra` relation.
///
/// The field types are distinct newtypes so the compiler rejects attempts
/// to use a chunk index as a relation index or vice versa — the confusion
/// at the root of the LSP highlight/hover clitic bugs fixed on 2026-04-16.
/// Serde and JSON schema wire formats remain plain integers thanks to
/// `#[serde(transparent)]` on the newtypes.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, JsonSchema, SpanShift)]
pub struct GraAlignmentPair {
    /// `%mor` chunk index (`None` means placeholder for extra `%gra` relation).
    pub mor_chunk_index: Option<MorChunkIndex>,

    /// `%gra` relation index (`None` means placeholder for extra `%mor` chunk).
    pub gra_index: Option<GraIndex>,
}

impl GraAlignmentPair {
    /// Builds one alignment row from optional typed indices.
    ///
    /// Complete rows use two `Some` indices; mismatch rows use one `None`.
    pub fn new(mor_chunk_index: Option<MorChunkIndex>, gra_index: Option<GraIndex>) -> Self {
        Self {
            mor_chunk_index,
            gra_index,
        }
    }

    /// Builds one alignment row from raw `usize` indices, wrapping each in
    /// the appropriate newtype. Convenience for call sites that still
    /// receive untyped positions from parser output.
    pub fn from_raw(mor_chunk_index: Option<usize>, gra_index: Option<usize>) -> Self {
        Self::new(
            mor_chunk_index.map(MorChunkIndex::new),
            gra_index.map(GraIndex::new),
        )
    }

    /// Returns `true` when this row is a complete one-to-one match.
    pub fn is_complete(&self) -> bool {
        self.mor_chunk_index.is_some() && self.gra_index.is_some()
    }

    /// Returns `true` when this row is a placeholder from count mismatch.
    pub fn is_placeholder(&self) -> bool {
        !self.is_complete()
    }
}

/// Generic trait impl exposes the underlying `usize` positions so code
/// written against [`IndexPair`](crate::alignment::IndexPair) keeps working
/// unchanged. New code should prefer the typed fields
/// (`.mor_chunk_index`, `.gra_index`) for compiler-enforced safety.
impl crate::alignment::traits::IndexPair for GraAlignmentPair {
    fn source(&self) -> Option<usize> {
        self.mor_chunk_index.map(MorChunkIndex::as_usize)
    }

    fn target(&self) -> Option<usize> {
        self.gra_index.map(GraIndex::as_usize)
    }

    fn from_indices(source: Option<usize>, target: Option<usize>) -> Self {
        Self::from_raw(source, target)
    }
}

impl crate::alignment::traits::TierAlignmentResult for GraAlignment {
    type Pair = GraAlignmentPair;

    fn pairs(&self) -> &[GraAlignmentPair] {
        &self.pairs
    }

    fn errors(&self) -> &[crate::ParseError] {
        &self.errors
    }

    fn push_pair(&mut self, pair: GraAlignmentPair) {
        self.pairs.push(pair);
    }

    fn push_error(&mut self, error: crate::ParseError) {
        self.errors.push(error);
    }
}
