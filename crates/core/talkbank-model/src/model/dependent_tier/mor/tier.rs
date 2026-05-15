//! `%mor` dependent-tier model and content validation helpers.
//!
//! This module defines the tier-level container used by parser output and
//! alignment logic, plus lexical-content checks shared by `%mor` validators.
//!
//! CHAT reference anchor:
//! - [Morphological tier](https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier)

use super::super::WriteChat;
use super::chunk::MorChunk;
use super::item::Mor;
use crate::alignment::indices::{GraHeadRef, MorItemIndex, SemanticWordIndex1};
use crate::model::content::Terminator;
use crate::model::dependent_tier::gra::{GraTier, GrammaticalRelation};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use talkbank_derive::{SemanticEq, SpanShift};

/// Type of morphological analysis tier.
///
/// The enum is intentionally explicit even though it currently has one variant.
/// This keeps tier-prefix logic uniform with other dependent-tier families.
///
/// # References
///
/// - [Morphological Tier](https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier)
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
pub enum MorTierType {
    /// Standard morphological analysis tier (%mor).
    Mor,
}

impl WriteChat for MorTierType {
    /// Writes the serialized tier tag used in CHAT files.
    ///
    /// Keeping this on `MorTierType` lets callers format tier prefixes without
    /// constructing a full [`MorTier`] value first.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        match self {
            MorTierType::Mor => w.write_str("%mor"),
        }
    }
}

/// Morphological analysis tier (%mor).
///
/// Provides word-by-word UD-style morphological annotation aligned with the main tier.
/// Each word receives a morphological code specifying part of speech, lemma,
/// and grammatical features.
///
/// # CHAT Format Example
///
/// ```text
/// *CHI: I want cookies .
/// %mor: pron|I-Prs-Nom-S1 verb|want-Fin-Ind-Pres-S1 noun|cookie-Plur .
/// ```
///
/// # Morphological Format
///
/// Each mor item has the UD structure: `POS|lemma[-Feature]*`
/// - **POS**: UD-style part-of-speech tag (e.g., `noun`, `verb`, `pron`)
/// - **Lemma**: Base form (e.g., `I`, `want`, `cookie`)
/// - **Features**: UD feature values (e.g., `-Plur`, `-Fin-Ind-Pres-S3`)
///
/// # Alignment
///
/// Mor tiers align 1-to-1 with alignable main tier content (words, not pauses/events).
/// See `crate::alignment::mor` for alignment algorithm.
///
/// # Terminator
///
/// Standard `%mor:` tiers end with a typed [`Terminator`] that matches the
/// main tier. In CA-mode, or for specialized fragments, the terminator may be
/// omitted if the main tier has no terminator.
///
/// # References
///
/// - [Morphological Tier](https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct MorTier {
    /// Type of morphological tier.
    pub tier_type: MorTierType,

    /// Morphological items aligned with main tier content.
    pub(crate) items: MorItems,

    /// Required terminator for this `%mor:` tier.
    pub terminator: Terminator,

    /// Source span for error reporting (not serialized to JSON)
    #[serde(skip)]
    #[schemars(skip)]
    pub span: crate::Span,
}

/// Errors returned by coordinated Mor-Gra mutations.
#[derive(Debug, thiserror::Error)]
pub enum CoordinatedMutationError {
    /// Mor and Gra tiers have mismatched chunk counts before or after mutation.
    #[error("Mor and Gra tiers have mismatched chunk counts: mor={mor}, gra={gra}")]
    CountMismatch {
        /// Chunk count in the %mor tier.
        mor: usize,
        /// Number of relations in the %gra tier.
        gra: usize,
    },
    /// Requested item index is out of bounds.
    #[error("Item index {index} out of bounds (len={len})")]
    ItemIndexOutOfBounds {
        /// The requested 0-indexed item position.
        index: usize,
        /// Total number of items in the tier.
        len: usize,
    },
    /// A new relation's head value is outside the new block but the
    /// caller's contract said it should be in span-relative-1-indexed
    /// space (i.e. within `1..=new_chunks`). Indicates a bug in the
    /// caller's head computation, not a misalignment of the host file.
    #[error(
        "New gra relation has head={head} but the new block has only \
         {new_chunks} chunks (heads must be in 1..={new_chunks} or 0)"
    )]
    HeadOutOfNewBlock {
        /// The offending head value.
        head: usize,
        /// Total chunk count of the new block.
        new_chunks: usize,
    },
    /// The host `%gra` tier is shorter than the chunk range the caller
    /// asked us to splice over. The caller passed a stale `item_range`
    /// or the host file has a pre-existing alignment defect that the
    /// caller should have detected first.
    #[error(
        "Host %gra tier has {gra_len} relations but splice needs {needed} \
         (chunk_offset={chunk_offset}, old_chunks={old_chunks})"
    )]
    GraTierTooShort {
        /// Number of relations currently in the host tier.
        gra_len: usize,
        /// Minimum needed: `chunk_offset + old_chunks`.
        needed: usize,
        /// Where the splice would start.
        chunk_offset: usize,
        /// How many chunks the splice would consume.
        old_chunks: usize,
    },
    /// A helper needed the `%gra` relation at a semantic chunk position but the
    /// host tier ended earlier.
    #[error(
        "Host %gra tier has {gra_len} relations but item start needs semantic index {semantic_index}"
    )]
    GraRelationMissing {
        /// The 1-indexed semantic chunk position that should have a matching
        /// `%gra` relation.
        semantic_index: SemanticWordIndex1,
        /// Number of relations currently in the host tier.
        gra_len: usize,
    },
}

impl MorTier {
    /// Construct a morphological tier with explicit type, items, and
    /// terminator.
    ///
    /// The constructor does not infer or validate alignment details. Those
    /// checks run later in dedicated validation/alignment stages.
    ///
    /// `terminator` is required because every well-formed `%mor:` tier has
    /// one. Parsers handling malformed input that lacks a terminator must
    /// return a typed parse-outcome diagnostic rather than calling this
    /// constructor with a placeholder.
    pub fn new(tier_type: MorTierType, items: Vec<Mor>, terminator: Terminator) -> Self {
        Self {
            tier_type,
            items: items.into(),
            terminator,
            span: crate::Span::DUMMY,
        }
    }

    /// Construct a standard `%mor` tier with the given items and
    /// terminator.
    ///
    /// This is the common constructor used by parser outputs and tests
    /// when no alternate tier kind is needed.
    pub fn new_mor(items: Vec<Mor>, terminator: Terminator) -> Self {
        Self::new(MorTierType::Mor, items, terminator)
    }

    /// Returns `true` when tier type is `%mor`.
    ///
    /// This helper is mainly useful in generic code paths over tier enums.
    pub fn is_mor(&self) -> bool {
        self.tier_type == MorTierType::Mor
    }

    /// Borrows the list of morphological items.
    pub fn items(&self) -> &[Mor] {
        &self.items.0
    }

    /// Consumes the tier and returns the underlying morphological items.
    pub fn into_items(self) -> Vec<Mor> {
        self.items.0
    }

    /// Mutably borrows the list of morphological items.
    ///
    /// # Safety
    ///
    /// Callers must ensure that any mutations to the items do not change their
    /// chunk counts (e.g., by adding or removing post-clitics), or that the
    /// corresponding %gra tier is updated to match.
    pub fn items_mut(&mut self) -> &mut [Mor] {
        &mut self.items.0
    }

    /// Replace a CONTIGUOUS RANGE of items and adjust the corresponding
    /// `%gra` relations atomically.
    ///
    /// This is the multi-item analog of [`Self::splice_coordinated`].
    /// The two methods share the same head-rewrite contract — heads in
    /// `1..=new_chunks` are within-block and remapped to
    /// `chunk_offset + head`; head 0 is the root anchor — but the range
    /// version interprets `new_chunks` as the SUM of chunks across all
    /// items in `new_mors`. This is what makes it correct for L2 spans
    /// covering multiple host words: the secondary Stanza sentence
    /// produces gras with cross-word heads (e.g. `la → fecha`), and the
    /// per-item splice path misclassifies those as within-MWT and
    /// remaps them with the wrong `chunk_offset`. See
    /// `docs/postmortems/2026-05-03-l2-splice-cardinality-investigation.md`
    /// §6c for the full failure trace.
    ///
    /// The `new_relations` list must have length equal to
    /// `sum(new_mors[i].count_chunks())`. Heads inside `new_relations`
    /// must be either `0` (span-internal root marker) or in
    /// `1..=sum(new_mors[i].count_chunks())` (span-relative within-block
    /// reference). Heads outside that range yield
    /// [`CoordinatedMutationError::HeadOutOfNewBlock`] — that contract
    /// holds the caller responsible for resolving any TRULY external
    /// references to host-absolute indices BEFORE calling this method,
    /// so the model layer never has to guess.
    ///
    /// Existing relations OUTSIDE the new block are reindexed and
    /// head-shifted by `delta = new_chunks - old_chunks`.
    ///
    /// Refuses (returns [`CoordinatedMutationError::GraTierTooShort`])
    /// when the host gra tier does not contain at least
    /// `chunk_offset + old_chunks` relations. We do NOT clamp silently —
    /// the postmortem (§6a, §6c) explicitly rejects soft-clamping, since
    /// it hides exactly the cardinality regressions this method exists
    /// to fix.
    pub fn splice_range_coordinated(
        &mut self,
        gra: &mut GraTier,
        item_range: std::ops::Range<usize>,
        new_mors: Vec<Mor>,
        new_relations: Vec<GrammaticalRelation>,
        root_anchor_override: Option<usize>,
    ) -> Result<(), CoordinatedMutationError> {
        if item_range.end > self.items.len() {
            return Err(CoordinatedMutationError::ItemIndexOutOfBounds {
                index: item_range.end,
                len: self.items.len(),
            });
        }

        // Old chunk count for the entire range.
        let old_chunks: usize = self.items.0[item_range.clone()]
            .iter()
            .map(|m| m.count_chunks())
            .sum();
        // New chunk count is the sum across all new mors.
        let new_chunks: usize = new_mors.iter().map(|m| m.count_chunks()).sum();

        if new_relations.len() != new_chunks {
            return Err(CoordinatedMutationError::CountMismatch {
                mor: new_chunks,
                gra: new_relations.len(),
            });
        }

        // Chunk offset of the first chunk in the range, host-1-indexed
        // would be chunk_offset + 1; here we keep 0-indexed for slice math.
        let chunk_offset: usize = self.items.0[..item_range.start]
            .iter()
            .map(|m| m.count_chunks())
            .sum();

        // Refuse to clamp: the host tier MUST cover the chunks we are
        // about to overwrite. Anything else is an upstream bug.
        let needed = chunk_offset + old_chunks;
        if needed > gra.relations.len() {
            return Err(CoordinatedMutationError::GraTierTooShort {
                gra_len: gra.relations.len(),
                needed,
                chunk_offset,
                old_chunks,
            });
        }

        // Validate every new relation's head BEFORE mutating anything,
        // so on error we leave both tiers unchanged.
        for rel in &new_relations {
            if rel.head != 0 && rel.head > new_chunks {
                return Err(CoordinatedMutationError::HeadOutOfNewBlock {
                    head: rel.head,
                    new_chunks,
                });
            }
        }

        let delta = (new_chunks as isize) - (old_chunks as isize);

        // 1. Update %mor items: replace the entire range with new_mors.
        self.items.0.splice(item_range, new_mors);

        // 2. Reindex the new relations to host-1-indexed.
        let mut fixed_relations = new_relations;
        for (i, rel) in fixed_relations.iter_mut().enumerate() {
            rel.index = chunk_offset + i + 1;
        }

        // 3. Capture the old head at the splice start before splicing it
        //    away — used as the default root anchor if no override is
        //    provided. (Same convention as splice_coordinated.)
        let old_head_at_start = gra.relations.0[chunk_offset].head;

        // 4. Splice the gra range.
        gra.relations
            .0
            .splice(chunk_offset..chunk_offset + old_chunks, fixed_relations);

        // 5. Reindex relations AFTER the new block so their `index`
        //    fields match their new tier position.
        if delta != 0 {
            for i in (chunk_offset + new_chunks)..gra.relations.len() {
                let rel = &mut gra.relations.0[i];
                rel.index = (rel.index as isize + delta) as usize;
            }
        }

        // 6. Adjust heads:
        //    - For new relations (positions [chunk_offset, chunk_offset+new_chunks)):
        //      head=0 → root_anchor_override or old_head_at_start
        //      head ∈ [1, new_chunks] → chunk_offset + head (within span)
        //    - For existing relations elsewhere:
        //      head pointing into the replaced range → collapse to chunk_offset + 1
        //      head pointing past the replaced range → shift by delta
        for (i, rel) in gra.relations.0.iter_mut().enumerate() {
            if i >= chunk_offset && i < chunk_offset + new_chunks {
                if rel.head == 0 {
                    rel.head = root_anchor_override.unwrap_or(old_head_at_start);
                } else {
                    // Already validated head ≤ new_chunks above, so this
                    // is always a within-block reference.
                    rel.head = chunk_offset + rel.head;
                }
                if rel.head == 0 {
                    rel.relation = "ROOT".into();
                }
            } else if rel.head > chunk_offset {
                if rel.head <= chunk_offset + old_chunks {
                    // Head was pointing into the range we replaced; the
                    // safest collapse target is the first chunk of the
                    // new block (matches splice_coordinated convention).
                    rel.head = chunk_offset + 1;
                } else {
                    // Head was pointing past the replaced range; shift
                    // by delta to keep pointing at the same conceptual
                    // chunk after the splice.
                    rel.head = (rel.head as isize + delta) as usize;
                }
            }
        }

        Ok(())
    }

    /// Replace the item at `item_idx` and adjust the corresponding `%gra`
    /// relations to maintain cardinality and index invariants.
    ///
    /// The `new_relations` list must match the chunk count of `new_mor`. This
    /// method handles re-indexing and head-adjustment for all subsequent
    /// relations in the `%gra` tier.
    pub fn splice_coordinated(
        &mut self,
        gra: &mut GraTier,
        item_idx: usize,
        new_mor: Mor,
        new_relations: Vec<GrammaticalRelation>,
        root_anchor_override: Option<usize>,
    ) -> Result<(), CoordinatedMutationError> {
        if item_idx >= self.items.len() {
            return Err(CoordinatedMutationError::ItemIndexOutOfBounds {
                index: item_idx,
                len: self.items.len(),
            });
        }

        let old_chunks = self.items[item_idx].count_chunks();
        let new_chunks = new_mor.count_chunks();

        if new_relations.len() != new_chunks {
            return Err(CoordinatedMutationError::CountMismatch {
                mor: new_chunks,
                gra: new_relations.len(),
            });
        }

        // Calculate the chunk offset for the item being replaced.
        let mut chunk_offset = 0usize;
        for i in 0..item_idx {
            chunk_offset += self.items[i].count_chunks();
        }

        let delta = (new_chunks as isize) - (old_chunks as isize);

        // 1. Update the %mor item.
        self.items.0[item_idx] = new_mor;

        // 2. Prepare the new relations with correct indices.
        let mut fixed_relations = new_relations;
        for (i, rel) in fixed_relations.iter_mut().enumerate() {
            rel.index = chunk_offset + i + 1;
        }

        // 3. Update the %gra relations list.
        let old_head = gra.relations.0[chunk_offset].head;

        gra.relations
            .0
            .splice(chunk_offset..chunk_offset + old_chunks, fixed_relations);

        // 4. Adjust indices and heads for the rest of the tier.
        if delta != 0 {
            let affected_start = chunk_offset + new_chunks;
            for i in affected_start..gra.relations.len() {
                let rel = &mut gra.relations.0[i];
                rel.index = (rel.index as isize + delta) as usize;
            }
        }

        // 5. Head adjustment for the whole tier.
        for (i, rel) in gra.relations.0.iter_mut().enumerate() {
            if i >= chunk_offset && i < chunk_offset + new_chunks {
                // This is one of the NEW relations.
                if rel.head == 0 {
                    // This was the root of the secondary block.
                    // It now points to the provided override, or falls back to the original head of the atom.
                    rel.head = root_anchor_override.unwrap_or(old_head);
                } else {
                    // Internal reference within the new block.
                    // Assumes secondary indices were 1-indexed relative to the block.
                    rel.head = chunk_offset + rel.head;
                }
                if rel.head == 0 {
                    rel.relation = "ROOT".into();
                }
            } else if rel.head > chunk_offset {
                // Existing relation pointing past the splice point.
                if rel.head <= chunk_offset + old_chunks {
                    // Head was pointing into the replaced item.
                    // Point to the first chunk of the new item.
                    rel.head = chunk_offset + 1;
                } else {
                    // Head was pointing past the replaced item.
                    rel.head = (rel.head as isize + delta) as usize;
                }
            }
        }

        Ok(())
    }

    /// Attach source span for diagnostics.
    ///
    /// Parser-generated values should set this to real offsets so `%mor` validation
    /// reports can point back to the original transcript line.
    pub fn with_span(mut self, span: crate::Span) -> Self {
        self.span = span;
        self
    }

    /// Count total number of chunks (including post-clitics and terminator).
    ///
    /// This is the canonical alignment unit for `%gra`: every chunk gets
    /// exactly one `%gra` relation. Equivalent to `self.chunks().count()`
    /// but implemented as a direct sum so the hot validation paths
    /// don't build an iterator chain just to discard it.
    ///
    pub fn count_chunks(&self) -> usize {
        let item_chunks: usize = self.items.iter().map(|m| m.count_chunks()).sum();
        item_chunks + 1
    }

    /// Iterate the `%mor` chunk sequence: each item's main word, then each of
    /// its post-clitics in serialized order, followed by the optional
    /// terminator.
    ///
    /// This is the **canonical** accessor for addressing `%gra` relation
    /// positions and for any downstream code that needs to project across
    /// `%mor`/`%gra` chunk boundaries (LSP hover, dependency-graph labels,
    /// CLI alignment rendering, diagnostic helpers). Every consumer crate
    /// MUST route through this method — walking `items` by hand silently
    /// drops post-clitics.
    pub fn chunks(&self) -> impl Iterator<Item = MorChunk<'_>> {
        self.items
            .iter()
            .flat_map(|item| {
                std::iter::once(MorChunk::Main(item)).chain(
                    item.post_clitics
                        .iter()
                        .map(move |clitic| MorChunk::PostClitic(item, clitic)),
                )
            })
            .chain(std::iter::once(MorChunk::Terminator(&self.terminator)))
    }

    /// Return the chunk at a 0-indexed position in the chunk sequence, or
    /// `None` if the index is out of range.
    ///
    /// `%gra` relation indices (`relation.index`, `relation.head`) are
    /// 1-indexed over this sequence, so a caller resolving a relation
    /// typically passes `word_index - 1`. A head value of `0` in `%gra` is
    /// reserved for ROOT and has no chunk — callers must handle that case
    /// before indexing.
    pub fn chunk_at(&self, chunk_index: usize) -> Option<MorChunk<'_>> {
        self.chunks().nth(chunk_index)
    }

    /// Project a 0-indexed chunk position to the 0-indexed `%mor` item that
    /// hosts it. Post-clitic chunks share the same host as their main word,
    /// so `item_index_of_chunk(0)` and `item_index_of_chunk(1)` both return
    /// `Some(0)` for a tier like `pron|it~aux|be noun|cookie`.
    ///
    /// Returns `None` for the terminator (not item-hosted) and for any
    /// out-of-range index.
    ///
    /// This is the exact primitive needed to project a `%gra` relation —
    /// whose alignment pair carries a chunk index — back through the
    /// main↔`%mor` alignment, which is keyed by item index. Consumers that
    /// attempt the projection without this method typically end up using
    /// the chunk index as if it were an item index, a bug class that was
    /// live in the LSP's gra-tier highlight handler until 2026-04-16.
    pub fn item_index_of_chunk(&self, chunk_index: usize) -> Option<usize> {
        let mut base = 0usize;
        for (item_idx, item) in self.items.iter().enumerate() {
            let span = item.count_chunks();
            if chunk_index < base + span {
                return Some(item_idx);
            }
            base += span;
        }
        None
    }

    /// Return the 1-indexed semantic `%gra` position of the first chunk owned by
    /// a `%mor` item.
    ///
    /// This is the typed bridge from the main↔`%mor` item alignment domain to
    /// the author-written `%gra` `index`/`head` space. For a post-cliticized
    /// item like `pron|it~aux|be`, the item start is semantic index `1`, while
    /// the item's governing syntactic head may still live on a later chunk.
    pub fn semantic_index_of_item_start(
        &self,
        item_idx: MorItemIndex,
    ) -> Option<SemanticWordIndex1> {
        let item_idx = item_idx.as_usize();
        (item_idx < self.items.len())
            .then(|| {
                self.items[..item_idx]
                    .iter()
                    .map(|item| item.count_chunks())
                    .sum::<usize>()
                    + 1
            })
            .and_then(|index| SemanticWordIndex1::new(index).ok())
    }

    /// Return the current `%gra` head for the first chunk of a `%mor` item.
    ///
    /// This is the shared "host governing chunk" seam for any code that needs
    /// to project a `%mor` item into the `%gra` dependency graph without
    /// confusing item indices with chunk indices. The returned [`GraHeadRef`]
    /// stays in the typed `%gra` head space and leaves the ROOT-vs-word branch
    /// explicit for the caller.
    pub fn governing_head_for_item(
        &self,
        gra: &GraTier,
        item_idx: MorItemIndex,
    ) -> Result<GraHeadRef, CoordinatedMutationError> {
        let semantic_index = self.semantic_index_of_item_start(item_idx).ok_or(
            CoordinatedMutationError::ItemIndexOutOfBounds {
                index: item_idx.as_usize(),
                len: self.items.len(),
            },
        )?;
        let relation = gra.relation_at_semantic_index(semantic_index).ok_or(
            CoordinatedMutationError::GraRelationMissing {
                semantic_index,
                gra_len: gra.len(),
            },
        )?;
        Ok(relation.head_ref())
    }

    /// Number of `%mor` items (excluding terminator/chunk expansion).
    ///
    /// Each item may still expand to multiple `%gra` chunks if it contains
    /// post-clitics.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns `true` when there are no `%mor` items.
    ///
    /// A tier with no items can still have a terminator; use [`Self::count_chunks`]
    /// when alignment logic needs the full chunk count.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Serialize full `%mor` line (`%mor:\t...`) to CHAT text.
    ///
    /// This writes prefix, items, and optional terminator in canonical order.
    /// It is the non-allocating path used by `WriteChat`.
    pub fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        match self.tier_type {
            MorTierType::Mor => w.write_str("%mor:\t")?,
        }

        for (i, item) in self.items.iter().enumerate() {
            if i > 0 {
                w.write_char(' ')?;
            }
            item.write_chat(w)?;
        }

        if !self.items.is_empty() {
            w.write_char(' ')?;
        }
        self.terminator.write_chat(w)?;

        Ok(())
    }

    /// Serialize full `%mor` line to an owned string.
    ///
    /// Prefer [`Self::write_chat`] when writing into existing buffers to avoid
    /// transient allocation.
    pub fn to_chat(&self) -> String {
        let mut s = String::new();
        let _ = self.write_chat(&mut s);
        s
    }

    /// Write tier content only (items and terminator), without the tier prefix (%mor:\t).
    ///
    /// This is used for roundtrip testing against golden data that contains
    /// content-only, and for the TreeSitterParser API which expects content-only input.
    pub fn write_content<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        for (i, item) in self.items.iter().enumerate() {
            if i > 0 {
                w.write_char(' ')?;
            }
            item.write_chat(w)?;
        }

        if !self.items.is_empty() {
            w.write_char(' ')?;
        }
        self.terminator.write_chat(w)?;

        Ok(())
    }

    /// Serialize content-only `%mor` payload to an owned string.
    ///
    /// This mirrors [`Self::write_content`] and is mainly a convenience for
    /// tests and debugging output.
    pub fn to_content(&self) -> String {
        let mut s = String::new();
        let _ = self.write_content(&mut s);
        s
    }
}

impl MorTier {
    /// Validate lexical content of all `%mor` items in this tier.
    ///
    /// Checks for empty POS/lemma/feature fields and reports `E711` diagnostics
    /// for each violation found.
    pub fn validate_content(&self, errors: &impl crate::ErrorSink) {
        validate_mor_content(&self.items, self.span, errors);
    }
}

impl WriteChat for MorTier {
    /// Serializes the full `%mor` line (prefix, items, optional terminator).
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        MorTier::write_chat(self, w)
    }
}

/// Newtype wrapper around a list of morphological items for a %mor tier.
///
/// # Reference
///
/// - [Morphological tier](https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct MorItems(pub(crate) Vec<Mor>);

impl MorItems {
    /// Create a new list of morphological items.
    ///
    /// Construction is intentionally lightweight so parser code can build the
    /// model first and run validation in a separate phase.
    pub fn new(items: Vec<Mor>) -> Self {
        Self(items)
    }

    /// Returns `true` if the list contains no items.
    ///
    /// This reflects only raw item count, not whether a parent tier has a
    /// terminator chunk.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Deref for MorItems {
    type Target = Vec<Mor>;

    /// Borrows the underlying `%mor` item vector.
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Vec<Mor>> for MorItems {
    /// Wraps `%mor` items without copying.
    fn from(items: Vec<Mor>) -> Self {
        Self(items)
    }
}

impl crate::validation::Validate for MorItems {
    /// Item-level constraints are enforced by `%mor` and alignment validators.
    fn validate(
        &self,
        _context: &crate::validation::ValidationContext,
        _errors: &impl crate::ErrorSink,
    ) {
    }
}

/// Validate content integrity of %mor items.
///
/// Checks every `MorWord` (in main words and post-clitics) for:
/// - E711: Empty lemma (`pos|` with no lemma after the pipe)
/// - E711: Empty POS category (`|lemma` with no POS before the pipe)
/// - E711: Empty feature (bare `-` separator with no text)
///
/// Structural alignment checks are intentionally out of scope here; this helper
/// only validates per-token lexical morphology content.
///
pub fn validate_mor_content(items: &[Mor], span: crate::Span, errors: &impl crate::ErrorSink) {
    use super::word::MorWord;
    use crate::{ErrorCode, ParseError, Severity};

    /// Validates a single `%mor` word for empty POS/lemma/feature fields.
    fn check_word(word: &MorWord, span: crate::Span, errors: &impl crate::ErrorSink) {
        if word.lemma.is_empty() {
            errors.report(
                ParseError::at_span(
                    ErrorCode::MorEmptyContent,
                    Severity::Error,
                    span,
                    format!("%mor word has empty lemma (POS='{}')", word.pos.as_str()),
                )
                .with_suggestion("Ensure the lemma is not empty"),
            );
        }
        if word.pos.is_empty() {
            errors.report(
                ParseError::at_span(
                    ErrorCode::MorEmptyContent,
                    Severity::Error,
                    span,
                    format!(
                        "%mor word has empty POS category (lemma='{}')",
                        word.lemma.as_str()
                    ),
                )
                .with_suggestion("Ensure the part-of-speech category is not empty"),
            );
        }
        for feature in &word.features {
            if feature.is_empty() {
                errors.report(
                    ParseError::at_span(
                        ErrorCode::MorEmptyContent,
                        Severity::Error,
                        span,
                        format!(
                            "%mor word has empty feature (lemma='{}')",
                            word.lemma.as_str()
                        ),
                    )
                    .with_suggestion("Remove the empty feature or provide feature text"),
                );
            }
        }
    }

    for item in items {
        // Check main word
        check_word(&item.main, span, errors);
        // Check post-clitics
        for clitic in &item.post_clitics {
            check_word(clitic, span, errors);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::analysis::{MorFeature, MorStem, PosCategory};
    use super::super::item::Mor;
    use super::super::word::MorWord;
    use super::*;
    use crate::alignment::{GraHeadRef, MorItemIndex, SemanticWordIndex1};
    use crate::{ErrorCode, ErrorCollector};

    /// Builds a `%mor` word fixture with given POS and lemma.
    ///
    /// Keeping this helper local avoids repetitive setup in each validation test.
    fn make_word(pos: &str, lemma: &str) -> MorWord {
        MorWord::new(PosCategory::new(pos), MorStem::new(lemma))
    }

    /// Wraps a word fixture into a single `%mor` item.
    ///
    /// Tests compose these items into tiers to exercise tier-level validators.
    fn make_mor(word: MorWord) -> Mor {
        Mor::new(word)
    }

    /// Builds a `%mor` tier fixture with a terminator.
    ///
    /// The terminator mirrors common corpus shape and keeps chunk accounting realistic.
    fn make_tier(items: Vec<Mor>) -> MorTier {
        MorTier::new_mor(
            items,
            crate::model::content::Terminator::Period {
                span: crate::Span::DUMMY,
            },
        )
    }

    /// Well-formed `%mor` content emits no `E711` diagnostics.
    ///
    /// This is the baseline guard for the validator's non-error path.
    #[test]
    fn test_mor_valid_content_no_errors() {
        let tier = make_tier(vec![
            make_mor(make_word("noun", "dog")),
            make_mor(make_word("verb", "run").with_feature(MorFeature::new("Past"))),
        ]);
        let errors = ErrorCollector::new();
        tier.validate_content(&errors);
        assert!(errors.into_vec().is_empty());
    }

    /// Empty lemma fields are rejected with `E711`.
    ///
    /// The message should explicitly mention the missing lemma component.
    #[test]
    fn test_mor_empty_lemma_produces_e711() {
        let tier = make_tier(vec![make_mor(make_word("noun", ""))]);
        let errors = ErrorCollector::new();
        tier.validate_content(&errors);
        let errs = errors.into_vec();
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].code, ErrorCode::MorEmptyContent);
        assert!(errs[0].message.contains("empty lemma"));
    }

    /// Empty POS categories are rejected with `E711`.
    ///
    /// This protects against malformed `|lemma` forms.
    #[test]
    fn test_mor_empty_pos_category_produces_e711() {
        let tier = make_tier(vec![make_mor(make_word("", "dog"))]);
        let errors = ErrorCollector::new();
        tier.validate_content(&errors);
        let errs = errors.into_vec();
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].code, ErrorCode::MorEmptyContent);
        assert!(errs[0].message.contains("empty POS category"));
    }

    /// Empty feature entries are rejected with `E711`.
    ///
    /// Bare `-` separators must not survive normalization.
    #[test]
    fn test_mor_empty_feature_produces_e711() {
        let word = make_word("verb", "walk").with_feature(MorFeature::new(""));
        let tier = make_tier(vec![make_mor(word)]);
        let errors = ErrorCollector::new();
        tier.validate_content(&errors);
        let errs = errors.into_vec();
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].code, ErrorCode::MorEmptyContent);
        assert!(errs[0].message.contains("empty feature"));
    }

    #[test]
    fn semantic_index_of_item_start_counts_prior_chunks() {
        let tier = make_tier(vec![
            make_mor(make_word("pron", "it")).with_post_clitic(make_word("aux", "be")),
            make_mor(make_word("noun", "cookie")),
        ]);

        assert_eq!(
            tier.semantic_index_of_item_start(MorItemIndex::new(0)),
            Some(SemanticWordIndex1::new(1).unwrap())
        );
        assert_eq!(
            tier.semantic_index_of_item_start(MorItemIndex::new(1)),
            Some(SemanticWordIndex1::new(3).unwrap())
        );
    }

    #[test]
    fn governing_head_for_item_uses_gra_chunk_space() {
        let tier = make_tier(vec![
            make_mor(make_word("pron", "it")).with_post_clitic(make_word("aux", "be")),
            make_mor(make_word("noun", "foreign")),
        ]);
        let gra = GraTier::new_gra(vec![
            GrammaticalRelation::new(1, 2, "EXPL"),
            GrammaticalRelation::new(2, 0, "ROOT"),
            GrammaticalRelation::new(3, 2, "DEP"),
            GrammaticalRelation::new(4, 2, "PUNCT"),
        ]);

        assert_eq!(
            tier.governing_head_for_item(&gra, MorItemIndex::new(1))
                .unwrap(),
            GraHeadRef::Word(SemanticWordIndex1::new(2).unwrap())
        );
    }

    #[test]
    fn splice_coordinated_normalizes_resulting_head_zero_relation_to_root() {
        let mut tier = make_tier(vec![make_mor(make_word("noun", "dog"))]);
        let mut gra = GraTier::new_gra(vec![
            GrammaticalRelation::new(1, 0, "ROOT"),
            GrammaticalRelation::new(2, 1, "PUNCT"),
        ]);

        tier.splice_coordinated(
            &mut gra,
            0,
            make_mor(make_word("noun", "woof")),
            vec![GrammaticalRelation::new(1, 0, "NMOD")],
            None,
        )
        .expect("splice_coordinated");

        let actual: Vec<String> = gra.relations().iter().map(ToString::to_string).collect();
        assert_eq!(
            actual,
            vec!["1|0|ROOT".to_string(), "2|1|PUNCT".to_string()]
        );
    }

    #[test]
    fn splice_range_coordinated_normalizes_resulting_head_zero_relation_to_root() {
        let mut tier = make_tier(vec![
            make_mor(make_word("noun", "old")),
            make_mor(make_word("noun", "tail")),
        ]);
        let mut gra = GraTier::new_gra(vec![
            GrammaticalRelation::new(1, 0, "ROOT"),
            GrammaticalRelation::new(2, 1, "NMOD"),
            GrammaticalRelation::new(3, 1, "PUNCT"),
        ]);

        tier.splice_range_coordinated(
            &mut gra,
            0..1,
            vec![make_mor(make_word("noun", "new"))],
            vec![GrammaticalRelation::new(1, 0, "NMOD")],
            None,
        )
        .expect("splice_range_coordinated");

        let actual: Vec<String> = gra.relations().iter().map(ToString::to_string).collect();
        assert_eq!(
            actual,
            vec![
                "1|0|ROOT".to_string(),
                "2|1|NMOD".to_string(),
                "3|1|PUNCT".to_string(),
            ]
        );
    }
}
