//! Structural-semantic equality utilities for CHAT model values.
//!
//! `SemanticEq` compares two parsed structures by linguistic content instead of
//! parse-time metadata. This lets roundtrip and migration tests assert "same
//! transcript semantics" even when spans, caches, or derived helper fields differ.
//!
//! Non-semantic fields are intentionally ignored by design:
//! - Source positions (`span` fields) via `impl SemanticEq for Span` returning `true`
//! - Computed alignment data (`alignments`) via `#[semantic_eq(skip)]`
//! - Derived language metadata (`language_metadata`) via `#[semantic_eq(skip)]`
//!
//! Reference: <https://talkbank.org/0info/manuals/CHAT.html#File_Format>

use smallvec::SmallVec;
use std::borrow::Cow;

/// Trait for semantic comparison of model values.
///
/// This trait is stricter than "rendered text equality" but looser than full
/// structural identity. Implementations should preserve linguistic meaning while
/// ignoring parse-time or cache-only fields that do not change corpus semantics.
pub trait SemanticEq {
    /// Returns whether `self` and `other` represent the same CHAT semantics.
    ///
    /// Callers typically use this in roundtrip tests where source spans and
    /// auxiliary runtime metadata are expected to differ after reparse.
    fn semantic_eq(&self, other: &Self) -> bool;
}

// =============================================================================
// Critical Implementation: Span is always Semantically Equal
// =============================================================================

impl SemanticEq for crate::Span {
    fn semantic_eq(&self, _other: &Self) -> bool {
        true
    }
}

// =============================================================================
// Blanket implementations for primitives
// =============================================================================

impl SemanticEq for String {
    fn semantic_eq(&self, other: &Self) -> bool {
        self == other
    }
}

impl SemanticEq for smol_str::SmolStr {
    fn semantic_eq(&self, other: &Self) -> bool {
        self == other
    }
}

impl SemanticEq for str {
    fn semantic_eq(&self, other: &Self) -> bool {
        self == other
    }
}

impl SemanticEq for &str {
    fn semantic_eq(&self, other: &Self) -> bool {
        *self == *other
    }
}

impl<'a> SemanticEq for Cow<'a, str> {
    fn semantic_eq(&self, other: &Self) -> bool {
        self.as_ref() == other.as_ref()
    }
}

impl SemanticEq for bool {
    fn semantic_eq(&self, other: &Self) -> bool {
        self == other
    }
}

impl SemanticEq for char {
    fn semantic_eq(&self, other: &Self) -> bool {
        self == other
    }
}

impl SemanticEq for u8 {
    fn semantic_eq(&self, other: &Self) -> bool {
        self == other
    }
}

impl SemanticEq for u16 {
    fn semantic_eq(&self, other: &Self) -> bool {
        self == other
    }
}

impl SemanticEq for u32 {
    fn semantic_eq(&self, other: &Self) -> bool {
        self == other
    }
}

impl SemanticEq for u64 {
    fn semantic_eq(&self, other: &Self) -> bool {
        self == other
    }
}

impl SemanticEq for usize {
    fn semantic_eq(&self, other: &Self) -> bool {
        self == other
    }
}

impl SemanticEq for i8 {
    fn semantic_eq(&self, other: &Self) -> bool {
        self == other
    }
}

impl SemanticEq for i16 {
    fn semantic_eq(&self, other: &Self) -> bool {
        self == other
    }
}

impl SemanticEq for i32 {
    fn semantic_eq(&self, other: &Self) -> bool {
        self == other
    }
}

impl SemanticEq for i64 {
    fn semantic_eq(&self, other: &Self) -> bool {
        self == other
    }
}

// =============================================================================
// Blanket implementations for containers
// =============================================================================

impl<T: SemanticEq> SemanticEq for Option<T> {
    fn semantic_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Some(a), Some(b)) => a.semantic_eq(b),
            (None, None) => true,
            _ => false,
        }
    }
}

impl<T: SemanticEq> SemanticEq for Vec<T> {
    fn semantic_eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().zip(other.iter()).all(|(a, b)| a.semantic_eq(b))
    }
}

impl<A: smallvec::Array> SemanticEq for SmallVec<A>
where
    A::Item: SemanticEq,
{
    fn semantic_eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().zip(other.iter()).all(|(a, b)| a.semantic_eq(b))
    }
}

impl<T: SemanticEq> SemanticEq for Box<T> {
    fn semantic_eq(&self, other: &Self) -> bool {
        (**self).semantic_eq(&**other)
    }
}

impl<T: SemanticEq + ?Sized> SemanticEq for std::sync::Arc<T> {
    fn semantic_eq(&self, other: &Self) -> bool {
        (**self).semantic_eq(&**other)
    }
}

impl<A: SemanticEq, B: SemanticEq> SemanticEq for (A, B) {
    fn semantic_eq(&self, other: &Self) -> bool {
        self.0.semantic_eq(&other.0) && self.1.semantic_eq(&other.1)
    }
}

impl<A: SemanticEq, B: SemanticEq, C: SemanticEq> SemanticEq for (A, B, C) {
    fn semantic_eq(&self, other: &Self) -> bool {
        self.0.semantic_eq(&other.0) && self.1.semantic_eq(&other.1) && self.2.semantic_eq(&other.2)
    }
}

impl<K: SemanticEq + Eq + std::hash::Hash, V: SemanticEq> SemanticEq for indexmap::IndexMap<K, V> {
    fn semantic_eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self
                .iter()
                .zip(other.iter())
                .all(|((k1, v1), (k2, v2))| k1.semantic_eq(k2) && v1.semantic_eq(v2))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `SemanticEq` for strings matches exact lexical equality.
    ///
    /// This is the primitive behavior other composite impls build upon.
    #[test]
    fn test_string_semantic_eq() {
        assert!("hello".to_string().semantic_eq(&"hello".to_string()));
        assert!(!"hello".to_string().semantic_eq(&"world".to_string()));
    }

    /// `SemanticEq` for `Option<T>` handles both value and `None` branches.
    ///
    /// The test covers equal values and mismatched `Some`/`None` combinations.
    #[test]
    fn test_option_semantic_eq() {
        assert!(Some("a".to_string()).semantic_eq(&Some("a".to_string())));
        assert!(Option::<String>::None.semantic_eq(&None));
        assert!(!Some("a".to_string()).semantic_eq(&None));
        assert!(!None.semantic_eq(&Some("b".to_string())));
    }

    /// `SemanticEq` for vectors is order-sensitive and length-sensitive.
    ///
    /// Different element values or shorter vectors must compare as non-equal.
    #[test]
    fn test_vec_semantic_eq() {
        let v1 = vec!["a".to_string(), "b".to_string()];
        let v2 = vec!["a".to_string(), "b".to_string()];
        let v3 = vec!["a".to_string(), "c".to_string()];
        let v4 = vec!["a".to_string()];

        assert!(v1.semantic_eq(&v2));
        assert!(!v1.semantic_eq(&v3));
        assert!(!v1.semantic_eq(&v4));
    }
}
