//! Typed index spaces for `%mor`/`%gra` alignment.
//!
//! `%mor`/`%gra` alignment involves four distinct integer "positions" that
//! used to share the same untyped `usize`. Confusing them produces bugs
//! that look like the code is working (the indices are `usize`, they
//! compile, the types match) but silently project to the wrong word on
//! any utterance containing a post-clitic. This module makes the four
//! spaces distinct types so the compiler rejects the confusion.
//!
//! | Type | 0- or 1-indexed | Sequence |
//! |------|-----------------|----------|
//! | [`MainWordIndex`] | 0-indexed | alignable words on the main tier |
//! | [`MorItemIndex`] | 0-indexed | [`MorTier::items`](crate::model::MorTier::items) |
//! | [`MorChunkIndex`] | 0-indexed | [`MorTier::chunks()`](crate::model::MorTier::chunks) (items expanded by post-clitics + terminator) |
//! | [`SemanticWordIndex1`] | **1-indexed** | `%gra` relation `index`/`head` fields |
//! | [`GraIndex`] | 0-indexed | [`GraTier::relations`](crate::model::GraTier::relations) |
//!
//! The enum [`GraHeadRef`] wraps `relation.head` explicitly, because
//! `head == 0` is the sentinel for ROOT and is *not* a valid
//! [`SemanticWordIndex1`]. Consumers that touch `relation.head` should
//! pass through `GraHeadRef::from_raw` before indexing anything.
//!
//! # References
//!
//! - [Morphological tier](https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier)
//! - [Grammatical relations tier](https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier)

use crate::SpanShift;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::num::NonZeroUsize;

/// Error returned when a `%gra` relation's `index` field is zero.
///
/// `index == 0` is not a valid semantic word position — 1 is the first word.
/// The only legal use of `0` in `%gra` is as the ROOT marker for
/// `relation.head`, which callers should route through [`GraHeadRef`]
/// instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("semantic word index must be 1-indexed (1..=N); got 0")]
pub struct SemanticWordIndexError;

macro_rules! zero_indexed_newtype {
    ($(#[$meta:meta])* $name:ident, $doc_unit:literal) => {
        $(#[$meta])*
        #[doc = ""]
        #[doc = concat!("Zero-indexed position into ", $doc_unit, ".")]
        #[derive(
            Clone,
            Copy,
            Debug,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Hash,
            Serialize,
            Deserialize,
            JsonSchema,
        )]
        #[serde(transparent)]
        pub struct $name(usize);

        impl $name {
            /// Construct a new index. No validation — any `usize` is
            /// representable; bounds are a property of the sequence being
            /// indexed, not of the index type.
            #[inline]
            pub const fn new(value: usize) -> Self {
                Self(value)
            }

            /// Return the raw `usize` for slice indexing or FFI.
            #[inline]
            pub const fn as_usize(self) -> usize {
                self.0
            }
        }

        impl From<usize> for $name {
            #[inline]
            fn from(value: usize) -> Self {
                Self::new(value)
            }
        }

        impl From<$name> for usize {
            #[inline]
            fn from(value: $name) -> usize {
                value.0
            }
        }

        impl std::fmt::Display for $name {
            #[inline]
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }

        // Newtypes wrap a plain integer and carry no source span, so
        // span shifting is a no-op. Required because pair types deriving
        // `SpanShift` recurse into their fields.
        impl SpanShift for $name {
            fn shift_spans_after(&mut self, _offset: u32, _delta: i32) {}
        }
    };
}

zero_indexed_newtype!(
    /// Position of an alignable word on the main tier.
    ///
    /// "Alignable" excludes pauses, events, terminator punctuation, and
    /// similar non-word content — see the [`TierDomain`](super::TierDomain)
    /// gating used by counters. The main tier is the source side of the
    /// main↔`%mor`/`%pho`/`%sin`/`%wor` alignments.
    MainWordIndex,
    "the alignable-word sequence of the main tier"
);

zero_indexed_newtype!(
    /// Position of a `%mor` item in [`MorTier::items`](crate::model::MorTier::items).
    ///
    /// One `%mor` item corresponds to one alignable main-tier word. An
    /// item's chunk count may exceed 1 when it carries post-clitics, but
    /// its main↔`%mor` alignment is still 1:1 on items — callers
    /// projecting from `%gra` chunks to the main tier must first collapse
    /// chunks to items via
    /// [`MorTier::item_index_of_chunk`](crate::model::MorTier::item_index_of_chunk).
    MorItemIndex,
    "the item sequence of a `%mor` tier"
);

zero_indexed_newtype!(
    /// Position of a chunk in the `%mor` chunk sequence (main + post-clitics
    /// + terminator).
    ///
    /// This is the alignment unit for `%gra`: each chunk gets one `%gra`
    /// relation. Chunk 1 of `pron|it~aux|be` is the `aux|be` post-clitic,
    /// which *shares the main-tier host word* with chunk 0 — projecting a
    /// chunk to its host item goes through
    /// [`MorTier::item_index_of_chunk`](crate::model::MorTier::item_index_of_chunk).
    MorChunkIndex,
    "the chunk sequence of a `%mor` tier"
);

zero_indexed_newtype!(
    /// Position of a relation in [`GraTier::relations`](crate::model::GraTier::relations).
    ///
    /// This is the *array position*, not the 1-indexed semantic word
    /// position written inside the relation's triple. Use
    /// [`SemanticWordIndex1`] when the value is the author-written position.
    GraIndex,
    "the relation sequence of a `%gra` tier"
);

zero_indexed_newtype!(
    /// Position of an item in [`PhoTier::items`](crate::model::PhoTier::items).
    ///
    /// `%pho` / `%mod` both use this same item-index space: the target
    /// side of the main↔`%pho` alignment is 1:1 with main-tier words.
    PhoItemIndex,
    "the item sequence of a `%pho` or `%mod` tier"
);

zero_indexed_newtype!(
    /// Position of a token in [`SinTier::items`](crate::model::SinTier::items).
    ///
    /// `%sin` aligns 1:1 with main-tier words (same structural rules as
    /// `%pho`); this newtype separates its indexing from the other tiers'
    /// so a future typed `AlignmentPair<MainWordIndex, SinItemIndex>`
    /// rejects cross-space confusion at compile time.
    SinItemIndex,
    "the item sequence of a `%sin` tier"
);

/// A 1-indexed semantic word position as used by `%gra` relations.
///
/// `%gra` writes relations like `2|1|AUX` where `2` is the dependent's
/// 1-indexed position in the `%mor` chunk sequence and `1` is the head's
/// position. `0` is not a valid dependent position — it is the ROOT
/// sentinel for `relation.head` only, represented by [`GraHeadRef::Root`].
///
/// Conversion: call [`Self::to_chunk_index`] to get the 0-indexed
/// [`MorChunkIndex`] for slice indexing.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct SemanticWordIndex1(NonZeroUsize);

impl SemanticWordIndex1 {
    /// Construct from a raw 1-indexed value. Returns an error if `value == 0`.
    #[inline]
    pub fn new(value: usize) -> Result<Self, SemanticWordIndexError> {
        NonZeroUsize::new(value)
            .map(Self)
            .ok_or(SemanticWordIndexError)
    }

    /// Construct from a [`NonZeroUsize`] directly, skipping the zero check.
    #[inline]
    pub const fn from_nonzero(value: NonZeroUsize) -> Self {
        Self(value)
    }

    /// Raw 1-indexed value.
    #[inline]
    pub const fn as_usize(self) -> usize {
        self.0.get()
    }

    /// Convert to a 0-indexed [`MorChunkIndex`] suitable for
    /// [`MorTier::chunk_at`](crate::model::MorTier::chunk_at).
    #[inline]
    pub const fn to_chunk_index(self) -> MorChunkIndex {
        MorChunkIndex::new(self.0.get() - 1)
    }
}

impl TryFrom<usize> for SemanticWordIndex1 {
    type Error = SemanticWordIndexError;

    #[inline]
    fn try_from(value: usize) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl std::fmt::Display for SemanticWordIndex1 {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.get().fmt(f)
    }
}

impl SpanShift for SemanticWordIndex1 {
    fn shift_spans_after(&mut self, _offset: u32, _delta: i32) {}
}

impl SpanShift for GraHeadRef {
    fn shift_spans_after(&mut self, _offset: u32, _delta: i32) {}
}

/// The head of a `%gra` relation: either ROOT (authored as `0`) or a
/// reference to another semantic word position.
///
/// Wrapping `relation.head` in this enum forces callers to handle the
/// ROOT case before attempting to index into any chunk sequence — a
/// common source of off-by-one-or-panic bugs when `head == 0` was
/// treated as a regular 1-indexed value.
///
/// Wire format: serializes as the raw integer (`0` for [`Self::Root`],
/// positive for [`Self::Word`]) so JSON consumers can read it as a
/// plain number without branching on a tag.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GraHeadRef {
    /// The relation's dependent is the root of the dependency tree.
    /// Authored as `0` in the CHAT `%gra` tier.
    Root,
    /// The relation's dependent depends on the word at this semantic
    /// position.
    Word(SemanticWordIndex1),
}

impl Serialize for GraHeadRef {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.as_raw().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for GraHeadRef {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = usize::deserialize(deserializer)?;
        Ok(Self::from_raw(raw))
    }
}

impl JsonSchema for GraHeadRef {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("GraHeadRef")
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        // Wire as a nonnegative integer: 0 means ROOT, positive is a 1-indexed word.
        <usize as JsonSchema>::json_schema(generator)
    }
}

impl GraHeadRef {
    /// Classify the raw `%gra` `head` field: `0` → [`Self::Root`],
    /// any positive value → [`Self::Word`].
    #[inline]
    pub fn from_raw(value: usize) -> Self {
        match SemanticWordIndex1::new(value) {
            Ok(idx) => Self::Word(idx),
            Err(_) => Self::Root,
        }
    }

    /// Return the underlying semantic position if this is not ROOT.
    #[inline]
    pub fn word(self) -> Option<SemanticWordIndex1> {
        match self {
            Self::Root => None,
            Self::Word(idx) => Some(idx),
        }
    }

    /// Serialize back to the raw `%gra` representation: `0` for ROOT,
    /// positive for a word reference.
    #[inline]
    pub fn as_raw(self) -> usize {
        match self {
            Self::Root => 0,
            Self::Word(idx) => idx.as_usize(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_indexed_newtypes_roundtrip() {
        let mor_item = MorItemIndex::new(3);
        assert_eq!(mor_item.as_usize(), 3);
        assert_eq!(usize::from(mor_item), 3);
        assert_eq!(MorItemIndex::from(3usize), mor_item);

        let mor_chunk = MorChunkIndex::new(1);
        assert_ne!(
            std::mem::size_of_val(&mor_item),
            0,
            "newtype should not collapse at runtime"
        );
        // These two types are distinct: assigning one to the other is a
        // compile error. The runtime roundtrip only verifies the `.as_usize()`
        // projection matches the construction value.
        assert_eq!(mor_chunk.as_usize(), 1);
    }

    #[test]
    fn semantic_word_index_rejects_zero() {
        assert!(SemanticWordIndex1::new(0).is_err());
        let idx = SemanticWordIndex1::new(2).expect("2 is valid");
        assert_eq!(idx.as_usize(), 2);
        assert_eq!(idx.to_chunk_index(), MorChunkIndex::new(1));
    }

    #[test]
    fn gra_head_ref_classifies_root_and_word() {
        assert_eq!(GraHeadRef::from_raw(0), GraHeadRef::Root);
        assert_eq!(GraHeadRef::from_raw(0).word(), None);
        assert_eq!(GraHeadRef::from_raw(0).as_raw(), 0);

        let word_ref = GraHeadRef::from_raw(3);
        assert!(matches!(word_ref, GraHeadRef::Word(_)));
        assert_eq!(word_ref.word().map(|i| i.as_usize()), Some(3));
        assert_eq!(word_ref.as_raw(), 3);
    }

    /// Serde roundtrip preserves the wire format: newtypes emit raw
    /// integers, `GraHeadRef` emits the raw integer on either branch.
    #[test]
    fn serde_roundtrip_preserves_raw_integers() {
        let chunk = MorChunkIndex::new(5);
        let json = serde_json::to_string(&chunk).unwrap();
        assert_eq!(json, "5");
        let round: MorChunkIndex = serde_json::from_str("5").unwrap();
        assert_eq!(round, chunk);

        let root = GraHeadRef::Root;
        assert_eq!(serde_json::to_string(&root).unwrap(), "0");

        let word = GraHeadRef::Word(SemanticWordIndex1::new(4).unwrap());
        assert_eq!(serde_json::to_string(&word).unwrap(), "4");
    }
}
