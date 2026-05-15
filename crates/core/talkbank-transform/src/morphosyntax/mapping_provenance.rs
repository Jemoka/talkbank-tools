//! Per-chunk provenance for sentence-level `%gra` construction.

use smallvec::SmallVec;
use talkbank_model::model::GrammaticalRelationType;

/// Per-chunk metadata produced by MOR synthesis and consumed by the `%gra`
/// builder.
#[derive(Debug, Clone)]
pub struct ChunkProvenance {
    /// UD word ids whose head references should resolve to this chunk.
    pub source_ud_ids: SmallVec<[usize; 1]>,
    /// Where this chunk's `%gra` head index should point.
    pub head: ChunkHead,
    /// The relation label for this chunk's `%gra` relation.
    pub deprel: GrammaticalRelationType,
}

/// How a chunk's head resolves to a concrete `%gra` head index.
#[derive(Debug, Clone)]
pub enum ChunkHead {
    /// The chunk is the sentence root.
    Root,
    /// Head resolves via the original UD head id.
    FromUd(usize),
    /// Head points to the main chunk of this provenance's owning MOR.
    OwningMorMain,
}

impl ChunkHead {
    /// Resolve a UD word's `.head` field into a `ChunkHead`.
    pub fn from_ud_head(ud_head: usize) -> Self {
        if ud_head == 0 {
            ChunkHead::Root
        } else {
            ChunkHead::FromUd(ud_head)
        }
    }
}

impl ChunkProvenance {
    /// Build provenance for a collapsed-Range chunk.
    pub fn collapsed_range(
        source_ud_ids: SmallVec<[usize; 1]>,
        head: ChunkHead,
        deprel: GrammaticalRelationType,
    ) -> Self {
        Self {
            source_ud_ids,
            head,
            deprel,
        }
    }

    /// Build provenance for a synthesized post-clitic chunk.
    pub fn synthetic_post_clitic(deprel: GrammaticalRelationType) -> Self {
        Self {
            source_ud_ids: SmallVec::new(),
            head: ChunkHead::OwningMorMain,
            deprel,
        }
    }
}

/// Per-MOR provenance list: chunk 0 is the main, rest are post-clitics.
pub type MorProvenance = SmallVec<[ChunkProvenance; 3]>;
