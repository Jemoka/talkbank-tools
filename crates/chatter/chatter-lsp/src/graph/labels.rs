//! `%mor` chunk label extraction for dependency graph nodes.
//!
//! Each chunk in the `%mor` chunk sequence (main word, post-clitic, or
//! terminator) becomes one graph node. The [`NodeLabel`] struct carries the
//! pre-rendered lemma text plus its 0-indexed chunk position, so
//! `graph::edges` can emit DOT without re-walking the tier. The walk itself
//! is delegated to [`MorTier::chunks`] — there is exactly one definition of
//! "the `%mor` chunk sequence" in the workspace, and this module is a
//! consumer of it, not a second implementation.

use talkbank_model::model::{MorChunk, MorTier};

/// Render metadata for one graph node derived from a `%mor` chunk.
pub(super) struct NodeLabel {
    /// 1-based DOT node identifier.
    pub(super) node_id: usize,
    /// Human-readable label (lemma for word chunks, punctuation for terminator).
    pub(super) label: String,
    /// 0-indexed position in the chunk sequence; used by edge lookup to
    /// match an alignment pair's `mor_chunk_index`.
    pub(super) chunk_idx: usize,
}

/// Collect node labels in morphology chunk order, including clitics and terminator.
///
/// Output invariants callers rely on:
///
/// - One entry per chunk, in chunk order — so `node_labels[i].chunk_idx == i`.
/// - `node_id` is 1-based and matches the DOT emission order.
/// - Word chunks carry their lemma; the terminator carries its literal text.
pub(super) fn collect_node_labels(mor_tier: &MorTier) -> Vec<NodeLabel> {
    mor_tier
        .chunks()
        .enumerate()
        .map(|(chunk_idx, chunk)| NodeLabel {
            node_id: chunk_idx + 1,
            label: label_for(&chunk),
            chunk_idx,
        })
        .collect()
}

/// Choose the display string for a chunk: lemma for word chunks, literal
/// terminator text otherwise.
fn label_for(chunk: &MorChunk<'_>) -> String {
    chunk
        .lemma()
        .map(str::to_owned)
        .or_else(|| chunk.terminator().map(|t| t.to_string()))
        .unwrap_or_default()
}
