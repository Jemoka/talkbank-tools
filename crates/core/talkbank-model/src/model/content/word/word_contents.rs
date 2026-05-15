//! [`WordContents`] — ordered sequence of [`WordContent`] elements.
//!
//! This module defines the [`SmallVec`]-backed newtype that holds a word's
//! internal structure. Most words are simple (1 element) or compounds (2–3).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use talkbank_derive::{SemanticEq, SpanShift};

use super::content::WordContent;

/// Ordered sequence of [`WordContent`] elements that make up a word's internal structure.
///
/// Backed by a [`SmallVec`] optimized for the common case of 1--2 elements.
/// Dereferences to a slice of [`WordContent`].
///
/// References:
/// - <https://talkbank.org/0info/manuals/CHAT.html#Words>
/// - <https://talkbank.org/0info/manuals/CHAT.html#Word_Tier>
#[derive(
    Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift, Default,
)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct WordContents(
    #[schemars(with = "Vec<WordContent>")] pub(crate) SmallVec<[WordContent; 2]>,
);

impl WordContents {
    /// Wraps parsed word-content elements without altering order.
    ///
    /// Callers are expected to provide already-normalized content; structural
    /// validation (for example empty-content checks) happens later.
    pub fn new(content: SmallVec<[WordContent; 2]>) -> Self {
        Self(content)
    }

    /// Returns `true` when this word has no content elements.
    ///
    /// Empty content is usually a parser-recovery artifact and is reported by
    /// validation as `E2xx`/empty-word-content style diagnostics.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Replace the element at `index` with `item`.
    ///
    /// # Panics
    /// Panics if `index` is out of bounds.
    pub fn replace_at(&mut self, index: usize, item: WordContent) {
        self.0[index] = item;
    }
}

impl std::ops::Deref for WordContents {
    type Target = SmallVec<[WordContent; 2]>;

    /// Borrows the underlying compact content sequence.
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Vec<WordContent>> for WordContents {
    /// Converts a heap vector into the small-vector-backed representation.
    fn from(content: Vec<WordContent>) -> Self {
        Self(SmallVec::from_vec(content))
    }
}

impl From<SmallVec<[WordContent; 2]>> for WordContents {
    /// Wraps an already-built `SmallVec` as `WordContents`.
    fn from(content: SmallVec<[WordContent; 2]>) -> Self {
        Self(content)
    }
}

impl<'a> IntoIterator for &'a WordContents {
    type Item = &'a WordContent;
    type IntoIter = std::slice::Iter<'a, WordContent>;

    /// Iterates borrowed word-content elements.
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'a> IntoIterator for &'a mut WordContents {
    type Item = &'a mut WordContent;
    type IntoIter = std::slice::IterMut<'a, WordContent>;

    /// Iterates mutable word-content elements.
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut()
    }
}

impl IntoIterator for WordContents {
    type Item = WordContent;
    type IntoIter = smallvec::IntoIter<[WordContent; 2]>;

    /// Consumes the wrapper and yields owned content elements.
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}
