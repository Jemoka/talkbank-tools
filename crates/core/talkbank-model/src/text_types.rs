//! Provenance-encoding newtype wrappers for text flowing through CHAT pipelines.
//!
//! `ChatCleanedText` and `ChatRawText` are **provenance-sealed**: the only way
//! to mint a value of either type is from a typed CHAT-AST node (a parsed
//! [`Word`] or [`Separator`]). There is no public string-accepting constructor,
//! by design.
//!
//! # Why provenance-sealed
//!
//! Cleaned text is, by definition, a computed view of a valid CHAT model — see
//! [`Word::compute_cleaned_text`] for the canonical projection (concatenated
//! `WordContent::Text` graphemes plus `WordContent::Shortening` restored
//! material; all prosodic/analytical markers excluded). "Cleaned" is meaningful
//! only relative to a structured source state. Constructing a `ChatCleanedText`
//! from a raw string is a category error: there is no source CHAT model the
//! string was cleaned *from*. The type system enforces this by making the
//! string-accepting constructor unavailable.
//!
//! Per-consumer projections downstream of `ChatCleanedText` (lowercasing for
//! frequency keys, apostrophe stripping for `wdlen`, etc.) are downstream
//! transforms — not different flavors of cleaned text. See
//! `<workspace>/docs/cleaned-text-consumer-policies.md` for the per-consumer
//! audit.
//!
//! See also `<workspace>/.claude/plans/migration-item-1-nutype-foundation-from-talkbank-model.md`
//! for the migration plan and `<workspace>/docs/architecture-review-2026-05-01.md`
//! Debt 11 for the architectural debt this change addresses.

use serde::{Deserialize, Serialize};
use std::fmt;

use crate::model::WriteChat;
use crate::model::content::Separator;
use crate::model::content::Word;

/// Generates the six `PartialEq` impls that let a provenance-sealed
/// string newtype be compared against `str`, `&str`, and `String` (in
/// either argument position). Comparison is read-only and does not
/// construct a value — this is ergonomics for tests and diagnostic
/// code, and does not undermine the constructor's provenance seal.
///
/// Used by [`ChatRawText`] and [`ChatCleanedText`].
macro_rules! impl_string_comparison_helpers {
    ($t:ty) => {
        impl PartialEq<str> for $t {
            fn eq(&self, other: &str) -> bool {
                self.0 == other
            }
        }
        impl PartialEq<&str> for $t {
            fn eq(&self, other: &&str) -> bool {
                self.0 == *other
            }
        }
        impl PartialEq<String> for $t {
            fn eq(&self, other: &String) -> bool {
                &self.0 == other
            }
        }
        impl PartialEq<$t> for str {
            fn eq(&self, other: &$t) -> bool {
                self == other.0.as_str()
            }
        }
        impl PartialEq<$t> for &str {
            fn eq(&self, other: &$t) -> bool {
                *self == other.0.as_str()
            }
        }
        impl PartialEq<$t> for String {
            fn eq(&self, other: &$t) -> bool {
                self == &other.0
            }
        }
    };
}

/// Source-faithful raw token text from a parsed CHAT AST node.
///
/// Includes CHAT markers and punctuation exactly as parsed, suitable for
/// roundtrip serialization and precise diagnostics. Provenance-sealed: only
/// constructible from a typed AST source — see module-level docs.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
#[repr(transparent)]
pub struct ChatRawText(String);

impl ChatRawText {
    /// Raw token text from a parsed [`Word`]. Mirrors [`Word::raw_text`].
    pub fn from_word_raw(word: &Word) -> Self {
        Self(word.raw_text().to_string())
    }

    /// Raw text projection of a parsed [`Separator`]. Separators have no
    /// raw/cleaned distinction — both projections render to the same CHAT
    /// surface form via [`WriteChat::to_chat_string`].
    pub fn from_separator(sep: &Separator) -> Self {
        Self(sep.to_chat_string())
    }

    /// Borrows the wrapped raw text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ChatRawText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for ChatRawText {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl ChatRawText {
    /// Test-only escape hatch. See [`ChatCleanedText::test_unchecked`]
    /// for full rationale.
    #[doc(hidden)]
    pub fn test_unchecked(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

// Read-only comparison helpers (see the macro definition below for
// rationale). Comparison cannot construct a value, so the seal stays
// intact — these are ergonomics for tests and log/diagnostic code.
impl_string_comparison_helpers!(ChatRawText);

/// Lexical content extracted from CHAT after markup stripping, projected from a
/// parsed CHAT AST node.
///
/// Cleaned text is the projection defined by [`Word::compute_cleaned_text`]:
/// concatenated `WordContent::Text` (base graphemes) and `WordContent::Shortening`
/// (elided material restored, e.g. `som(e)` → `some`). All prosodic/analytical
/// markers are excluded.
///
/// Provenance-sealed: only constructible from a typed AST source — see
/// module-level docs.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
#[repr(transparent)]
pub struct ChatCleanedText(String);

impl ChatCleanedText {
    /// Cleaned-text projection of a parsed [`Word`]. Delegates to
    /// [`Word::cleaned_text`] (which itself caches the result of
    /// [`Word::compute_cleaned_text`]).
    pub fn from_word(word: &Word) -> Self {
        Self(word.cleaned_text().to_string())
    }

    /// Cleaned-text projection of a parsed [`Separator`]. A separator's
    /// cleaned form is its CHAT surface — there are no markers to strip.
    pub fn from_separator(sep: &Separator) -> Self {
        Self(sep.to_chat_string())
    }

    /// Synthetic placeholder substituted for special-form (`@<letter>`,
    /// excluding `@s`) surface text before Stanza dispatch. Stanza sees the
    /// placeholder, not the non-word, so the surrounding parse stays clean.
    /// The post-Stanza synthesis pass replaces the placeholder's analysis
    /// with form-type-derived MOR.
    ///
    /// This is the **only** blessed exception to provenance sealing — it is
    /// not derived from a parsed AST node, but its constant value is part of
    /// the morphotag-pipeline contract and tokenizes as a single word in
    /// every Stanza language pack. See
    /// `crates/talkbank-transform/src/morphosyntax/payload.rs` for usage,
    /// and `crates/talkbank-transform/src/morphosyntax/synthesis/` for the
    /// post-Stanza recognition + replacement.
    pub fn stanza_placeholder() -> Self {
        Self("xbxxx".to_string())
    }

    /// Borrows the wrapped cleaned text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns an iterator over the characters of the cleaned text.
    pub fn chars(&self) -> std::str::Chars<'_> {
        self.0.chars()
    }

    /// Lowercases the cleaned text.
    pub fn to_lowercase(&self) -> String {
        self.0.to_lowercase()
    }
}

impl fmt::Display for ChatCleanedText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for ChatCleanedText {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl ChatCleanedText {
    /// Test-only escape hatch. Constructs a `ChatCleanedText` from an
    /// arbitrary string without going through a parsed AST node.
    ///
    /// The `test_` prefix and `#[doc(hidden)]` flag this as a
    /// fixture-construction tool. Production code must construct
    /// `ChatCleanedText` via [`Self::from_word`],
    /// [`Self::from_separator`], or [`Self::stanza_placeholder`].
    ///
    /// Tests use this to construct `ChatCleanedText` fixtures for
    /// downstream logic (alignment, dispatch, retokenization) without
    /// paying the cost of running the full parse pipeline. The caller
    /// is responsible for passing a string that would be a valid
    /// cleaned-text projection if it had come from a parse.
    #[doc(hidden)]
    pub fn test_unchecked(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

impl_string_comparison_helpers!(ChatCleanedText);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::content::Word;

    #[test]
    fn cleaned_and_raw_text_remain_distinct() {
        // `Word::new_unchecked` builds a `Word` whose `cleaned_text()` is the
        // second argument, derived through the `compute_cleaned_text` projection
        // on a single-`Text`-element `WordContents`. The raw_text and cleaned
        // values can legitimately differ (e.g., raw "hello@c" with cleaned
        // "hello") because `WordContents` carries cleaned material only.
        let word = Word::new_unchecked("hello@c", "hello");
        let raw = ChatRawText::from_word_raw(&word);
        let cleaned = ChatCleanedText::from_word(&word);

        assert_eq!(raw.as_str(), "hello@c");
        assert_eq!(cleaned.as_str(), "hello");
    }

    #[test]
    fn stanza_placeholder_value_is_pinned() {
        // The placeholder string is a contract with the post-Stanza
        // synthesis pass in `crates/talkbank-transform/src/morphosyntax/synthesis/`,
        // which recognizes this exact value to identify substituted
        // special-form positions. Changing it would silently break the
        // round-trip. Pinning the value here surfaces any change as a
        // failing test.
        assert_eq!(ChatCleanedText::stanza_placeholder().as_str(), "xbxxx");
    }

    #[test]
    fn cleaned_text_serializes_transparently() {
        let word = Word::new_unchecked("test", "test");
        let text = ChatCleanedText::from_word(&word);
        let json = serde_json::to_string(&text).unwrap();
        assert_eq!(json, "\"test\"");

        let decoded: ChatCleanedText = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, text);
    }
}
