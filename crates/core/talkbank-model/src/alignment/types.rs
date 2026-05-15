//! Common index-pair primitives shared by tier-alignment passes.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Word_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier>

use schemars::JsonSchema;
use talkbank_derive::SpanShift;

/// One positional mapping entry between a source tier and a target tier.
///
/// Most entries are complete `Some -> Some` matches. Placeholder entries with
/// one `None` preserve mismatch shape (extra/missing units) so diagnostics can
/// still report concrete positions for both tiers.
///
/// # Typed variants
///
/// `AlignmentPair` is generic over the source and target index types so a
/// per-tier result type can declare the concrete index space without writing
/// its own pair struct:
///
/// - `AlignmentPair<MainWordIndex, MorItemIndex>` — main↔`%mor`
/// - `AlignmentPair<MainWordIndex, PhoItemIndex>` — main↔`%pho` / main↔`%mod`
/// - `AlignmentPair<MainWordIndex, SinItemIndex>` — main↔`%sin`
///
/// `%gra` is a `%mor`↔`%gra` pairing and uses its own typed pair
/// [`GraAlignmentPair`](super::GraAlignmentPair), not this one. `%wor` is
/// not a structural alignment — see
/// [`WorTimingSidecar`](super::WorTimingSidecar).
///
/// The type parameters default to `usize` so pre-existing code written
/// against `AlignmentPair` (without generic arguments) continues to compile
/// unchanged.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, JsonSchema, SpanShift)]
pub struct AlignmentPair<S = usize, T = usize> {
    /// Index in the source tier (for example the main tier), or `None` for placeholder rows.
    pub source_index: Option<S>,
    /// Index in the target tier (for example `%mor`/`%pho`/`%wor`), or `None` for placeholders.
    pub target_index: Option<T>,
}

impl<S, T> AlignmentPair<S, T> {
    /// Builds one alignment row from optional source/target indices.
    ///
    /// Callers usually pass two `Some` values for normal matches, or one `None`
    /// when recording an insertion/deletion style mismatch.
    pub fn new(source_index: Option<S>, target_index: Option<T>) -> Self {
        Self {
            source_index,
            target_index,
        }
    }

    /// Returns `true` when this row is a concrete one-to-one match.
    ///
    /// Complete rows are the ones eligible for downstream pairwise joins.
    pub fn is_complete(&self) -> bool {
        self.source_index.is_some() && self.target_index.is_some()
    }

    /// Returns `true` when this row represents an unmatched position.
    ///
    /// Placeholder rows are emitted only for mismatches and are expected when
    /// tiers have different alignable counts.
    pub fn is_placeholder(&self) -> bool {
        !self.is_complete()
    }
}

/// The generic `IndexPair` trait uses raw `usize` source/target indices so
/// code that predates KIB-001 (or consumers that deliberately want a
/// tier-agnostic view, like the shared `find_source_index_for_target` helper)
/// keeps working. The bound `Copy + From<usize> + Into<usize>` is satisfied
/// by `usize` itself and by every domain index newtype in
/// [`super::indices`].
impl<S, T> super::traits::IndexPair for AlignmentPair<S, T>
where
    S: Copy + From<usize> + Into<usize>,
    T: Copy + From<usize> + Into<usize>,
{
    fn source(&self) -> Option<usize> {
        self.source_index.map(Into::into)
    }

    fn target(&self) -> Option<usize> {
        self.target_index.map(Into::into)
    }

    fn from_indices(source: Option<usize>, target: Option<usize>) -> Self {
        Self::new(source.map(From::from), target.map(From::from))
    }
}
