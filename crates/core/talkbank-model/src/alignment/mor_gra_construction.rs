//! Co-construction primitive for `(MorTier, GraTier)` pairs that
//! enforces alignment as a constructor invariant.
//!
//! [`try_align_mor_gra`] takes word-side mors, word-side gra
//! relations, and a [`MorGraTerminatorSlot`] (the terminator and its
//! paired dependency relation), validates that chunks and relations
//! line up, and produces both tiers together. Either both come out
//! aligned, or construction fails with a typed
//! [`MorGraConstructionError`].

use crate::Span;
use crate::model::content::Terminator;
use crate::model::dependent_tier::{GraTier, GrammaticalRelation, Mor, MorTier};

/// Terminator and its paired `%gra` relation, traveling together.
///
/// Both halves are required to construct a paired `(MorTier, GraTier)`
/// via [`try_align_mor_gra`]; supplying the morphology terminator
/// without its dependency relation (or vice versa) is impossible at
/// the type level.
#[derive(Clone, Debug, PartialEq)]
pub struct MorGraTerminatorSlot {
    /// Typed CHAT terminator (`.`, `?`, `!`, CA-prosody arrow, …).
    pub terminator: Terminator,
    /// `%gra` relation paired with the terminator slot. Conventionally
    /// `index = total_chunks + 1, head = root_chunk_idx, relation = PUNCT`.
    pub relation: GrammaticalRelation,
}

/// Construction-time error class for the co-construction primitive.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum MorGraConstructionError {
    /// Item-side chunk count from `MorItem`s does not match the
    /// item-side `GrammaticalRelation` count.
    #[error("%mor/%gra count mismatch: {mor_chunks} mor chunks vs {gra_relations} gra relations")]
    CountMismatch {
        /// Total chunks across `mor_items` (sum of `Mor::count_chunks`).
        mor_chunks: usize,
        /// Number of supplied item-side `GrammaticalRelation`s.
        gra_relations: usize,
    },
}

/// Build paired `(MorTier, GraTier)` enforcing chunk-relation alignment.
///
/// `item_relations` covers word/clitic chunks only and excludes the
/// terminator's relation; the terminator slot is supplied separately
/// as a [`MorGraTerminatorSlot`]. Returns `(MorTier, GraTier)` where
/// `mor.count_chunks() == gra.len()` is guaranteed by construction.
pub fn try_align_mor_gra(
    mor_items: Vec<Mor>,
    item_relations: Vec<GrammaticalRelation>,
    terminator_slot: MorGraTerminatorSlot,
    span: Span,
) -> Result<(MorTier, GraTier), MorGraConstructionError> {
    let mor_chunks: usize = mor_items.iter().map(|m| m.count_chunks()).sum();
    if mor_chunks != item_relations.len() {
        return Err(MorGraConstructionError::CountMismatch {
            mor_chunks,
            gra_relations: item_relations.len(),
        });
    }
    let MorGraTerminatorSlot {
        terminator,
        relation: terminator_relation,
    } = terminator_slot;
    let mor = MorTier::new_mor(mor_items, terminator).with_span(span);
    let mut all_relations = item_relations;
    all_relations.push(terminator_relation);
    let gra = GraTier::new_gra(all_relations).with_span(span);
    Ok((mor, gra))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::dependent_tier::{MorWord, PosCategory};

    fn simple_mor(pos: &str, lemma: &str) -> Mor {
        Mor::new(MorWord::new(PosCategory::new(pos), lemma))
    }

    fn term_slot(index: usize, head: usize) -> MorGraTerminatorSlot {
        MorGraTerminatorSlot {
            terminator: Terminator::Period { span: Span::DUMMY },
            relation: GrammaticalRelation::new(index, head, "PUNCT"),
        }
    }

    /// Equal counts produce aligned tiers.
    #[test]
    fn aligned_inputs_succeed() {
        let mors = vec![simple_mor("pron", "I"), simple_mor("verb", "go")];
        let item_relations = vec![
            GrammaticalRelation::new(1, 2, "NSUBJ"),
            GrammaticalRelation::new(2, 0, "ROOT"),
        ];
        let (mor, gra) =
            try_align_mor_gra(mors, item_relations, term_slot(3, 2), Span::DUMMY).expect("aligns");
        assert_eq!(mor.count_chunks(), gra.len());
        assert_eq!(mor.count_chunks(), 3);
    }

    /// Item-side chunk-count mismatch produces a typed CountMismatch error.
    #[test]
    fn mismatched_counts_fail() {
        let mors = vec![simple_mor("pron", "I")];
        let item_relations = vec![
            GrammaticalRelation::new(1, 0, "ROOT"),
            GrammaticalRelation::new(2, 0, "EXTRA"),
        ];
        let err = try_align_mor_gra(mors, item_relations, term_slot(3, 1), Span::DUMMY)
            .expect_err("should mismatch");
        assert_eq!(
            err,
            MorGraConstructionError::CountMismatch {
                mor_chunks: 1,
                gra_relations: 2,
            }
        );
    }

    /// MWT-joined Mor (one item, two chunks) requires two item_relations.
    #[test]
    fn mwt_chunks_require_matching_relations() {
        // pro|it~aux|be — one Mor, two chunks (main + post-clitic).
        let it = Mor::new(MorWord::new(PosCategory::new("pron"), "it"))
            .with_post_clitic(MorWord::new(PosCategory::new("aux"), "be"));
        let item_relations = vec![
            GrammaticalRelation::new(1, 0, "ROOT"),
            GrammaticalRelation::new(2, 1, "AUX"),
        ];
        let (mor, gra) = try_align_mor_gra(vec![it], item_relations, term_slot(3, 1), Span::DUMMY)
            .expect("aligns");
        assert_eq!(mor.count_chunks(), 3);
        assert_eq!(gra.len(), 3);
    }

    /// Terminator-only utterance (zero items) succeeds and produces a
    /// length-1 GraTier (just the terminator's PUNCT).
    #[test]
    fn empty_items_succeed() {
        let (mor, gra) =
            try_align_mor_gra(vec![], vec![], term_slot(1, 0), Span::DUMMY).expect("aligns");
        assert_eq!(mor.count_chunks(), 1);
        assert_eq!(gra.len(), 1);
    }

    /// Caller bug: passing item_relations that already include the
    /// terminator's PUNCT entry must be caught (item_relations.len()
    /// would exceed mor_chunks by one). The CountMismatch check fires.
    #[test]
    fn caller_double_includes_terminator_is_caught() {
        let mors = vec![simple_mor("pron", "I")];
        // Caller mistakenly includes terminator's PUNCT relation in
        // item_relations alongside the proper chunk-relation. Two
        // entries vs one mor chunk — function rejects.
        let item_relations = vec![
            GrammaticalRelation::new(1, 0, "ROOT"),
            GrammaticalRelation::new(2, 1, "PUNCT"),
        ];
        let err = try_align_mor_gra(mors, item_relations, term_slot(3, 1), Span::DUMMY)
            .expect_err("caller mistake should be caught");
        assert_eq!(
            err,
            MorGraConstructionError::CountMismatch {
                mor_chunks: 1,
                gra_relations: 2,
            }
        );
    }
}
