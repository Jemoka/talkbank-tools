//! Core `%mor` lexical/morphological atom types.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Part_of_Speech>

use std::borrow::Cow;
use std::sync::Arc;

use crate::SpanShift;
use crate::interned_newtype;
use crate::model::SemanticEq;
use crate::model::semantic_diff::{
    SemanticDiff, SemanticDiffContext, SemanticDiffKind, SemanticDiffReport, SemanticPath,
};

// =============================================================================
// Newtype Definitions Using Global interned_newtype! Macro
// =============================================================================

interned_newtype! {
    /// Part-of-speech category (e.g., `noun` for noun, `verb` for verb, `pron` for pronoun).
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Part_of_Speech>
    ///
    /// ## Memory Optimization
    ///
    /// This type uses `Arc<str>` with interning for memory efficiency:
    /// - All categories are interned through a global interner
    /// - Common categories (noun, verb, pron, det, etc.) are pre-populated on first use
    /// - Cloning is O(1) (atomic reference count increment)
    /// - Multiple occurrences of the same category share a single Arc
    ///
    /// This reduces memory usage by 5-30MB for large corpora with extensive %mor tiers.
    pub struct PosCategory,
    interner: crate::model::pos_interner()
}

interned_newtype! {
    /// Morphological stem/lemma (e.g., `go`, `run`, `child`).
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
    ///
    /// ## Memory Optimization
    ///
    /// This type uses `Arc<str>` with interning for memory efficiency:
    /// - All stems are interned through a global interner
    /// - Common stems (the, a, be, have, do, etc.) are pre-populated on first use
    /// - Cloning is O(1) (atomic reference count increment)
    /// - Multiple occurrences of the same stem share a single Arc
    ///
    /// This reduces memory usage by 5-30MB for large corpora.
    pub struct MorStem,
    interner: crate::model::stem_interner()
}

// =============================================================================
// MorFeature — Hand-Written Struct with Optional Key=Value
// =============================================================================

/// Morphological feature with optional key-value structure.
///
/// Supports both flat features (`Plur`, `Past`) and keyed UD features
/// (`Number=Plur`, `Tense=Past`). When parsing, if the input contains `=`,
/// it splits into key+value; otherwise it's a flat feature.
///
/// ## Roundtrip Guarantee
///
/// - Input: `-Plur` → `MorFeature { key: None, value: "Plur" }` → Output: `-Plur`
/// - Input: `-Number=Plur` → `MorFeature { key: Some("Number"), value: "Plur" }` → Output: `-Number=Plur`
/// - Input: `-S3` → `MorFeature { key: None, value: "S3" }` → Output: `-S3`
/// - Input: `-Int,Rel` → `MorFeature { key: None, value: "Int,Rel" }` → Output: `-Int,Rel`
///
/// No feature decomposition, no key inference, no format normalization.
///
/// ## Memory Optimization
///
/// Both key and value use `Arc<str>` with interning for memory efficiency.
///
/// References:
/// - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
/// - <https://talkbank.org/0info/manuals/CHAT.html#Part_of_Speech>
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MorFeature {
    key: Option<Arc<str>>,
    value: Arc<str>,
}

impl MorFeature {
    /// Create a new feature, auto-detecting key=value format.
    ///
    /// If `s` contains `=`, splits on the first `=` into key and value.
    /// Otherwise, creates a flat (keyless) feature.
    pub fn new(s: impl AsRef<str>) -> Self {
        let s = s.as_ref();
        let interner = crate::model::stem_interner();
        if let Some(eq_pos) = s.find('=') {
            Self {
                key: Some(interner.intern(&s[..eq_pos])),
                value: interner.intern(&s[eq_pos + 1..]),
            }
        } else {
            Self {
                key: None,
                value: interner.intern(s),
            }
        }
    }

    /// Create a flat (keyless) feature.
    ///
    /// Use this when callers already know the feature is not keyed and want to
    /// avoid extra parsing logic in `new`.
    pub fn flat(value: impl AsRef<str>) -> Self {
        Self {
            key: None,
            value: crate::model::stem_interner().intern(value.as_ref()),
        }
    }

    /// Create a keyed feature (key=value).
    ///
    /// This constructor preserves both key and value verbatim and does not
    /// normalize case or spelling.
    pub fn with_key_value(key: impl AsRef<str>, value: impl AsRef<str>) -> Self {
        let interner = crate::model::stem_interner();
        Self {
            key: Some(interner.intern(key.as_ref())),
            value: interner.intern(value.as_ref()),
        }
    }

    /// Returns the key, if present (e.g., `Some("Number")` for `Number=Plur`).
    ///
    /// Flat features such as `Plur` return `None` here and keep all content in
    /// [`Self::value`].
    pub fn key(&self) -> Option<&str> {
        self.key.as_deref()
    }

    /// Returns the value (e.g., `"Plur"` for both `Plur` and `Number=Plur`).
    ///
    /// This always returns the right-hand payload, regardless of whether the
    /// feature was keyed or flat.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns `true` if this is a flat feature (no key).
    ///
    /// Flat vs keyed status is lexical only; no UD schema lookup is performed.
    pub fn is_flat(&self) -> bool {
        self.key.is_none()
    }

    /// Returns `true` if the value is empty.
    ///
    /// Empty values are representable at model level and are typically surfaced
    /// later by validation rules.
    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }
}

// --- Serde: serialize/deserialize as string ---

impl serde::Serialize for MorFeature {
    /// Serializes as the canonical `%mor` surface form (`value` or `key=value`).
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match &self.key {
            Some(k) => {
                let full = format!("{}={}", k, self.value);
                serializer.serialize_str(&full)
            }
            None => serializer.serialize_str(&self.value),
        }
    }
}

impl<'de> serde::Deserialize<'de> for MorFeature {
    /// Deserializes from `%mor` surface text, preserving optional key/value form.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(MorFeature::new(s))
    }
}

// --- JsonSchema: "type": "string" (unchanged wire format) ---

impl schemars::JsonSchema for MorFeature {
    /// Stable schema type name used in generated JSON schema.
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("MorFeature")
    }

    /// Advertises wire format as a JSON string.
    fn json_schema(_gen: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({ "type": "string" })
    }
}

// --- SemanticEq / SpanShift ---

impl SemanticEq for MorFeature {
    /// Semantic equivalence matches exact key/value lexical content.
    fn semantic_eq(&self, other: &Self) -> bool {
        self.key.semantic_eq(&other.key) && self.value.semantic_eq(&other.value)
    }
}

impl SpanShift for MorFeature {
    /// Shifts spans after.
    fn shift_spans_after(&mut self, _offset: u32, _delta: i32) {
        // No span fields
    }
}

// --- SemanticDiff ---

impl SemanticDiff for MorFeature {
    /// Emits a value mismatch when two features differ textually.
    fn semantic_diff_into(
        &self,
        other: &Self,
        path: &mut SemanticPath,
        report: &mut SemanticDiffReport,
        ctx: &mut SemanticDiffContext,
    ) {
        if self != other {
            report.push_with_context(
                path,
                SemanticDiffKind::ValueMismatch,
                format!("{self}"),
                format!("{other}"),
                ctx,
            );
        }
    }
}

// --- WriteChat / Display ---

impl crate::model::WriteChat for MorFeature {
    /// Writes this feature in canonical CHAT `%mor` form.
    ///
    /// Keyed features emit `key=value`; flat features emit only `value`.
    /// Serialization is lossless with respect to stored lexical content.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        if let Some(ref k) = self.key {
            w.write_str(k)?;
            w.write_char('=')?;
        }
        w.write_str(&self.value)
    }
}

impl std::fmt::Display for MorFeature {
    /// Formats as `value` or `key=value`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(ref k) = self.key {
            write!(f, "{}={}", k, self.value)
        } else {
            f.write_str(&self.value)
        }
    }
}

// --- From conversions ---

impl From<String> for MorFeature {
    /// Builds a feature from owned string input.
    ///
    /// The parser path is identical to [`MorFeature::new`], so `=` detection and
    /// interning behavior are preserved.
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for MorFeature {
    /// Builds a feature from borrowed string input.
    ///
    /// This exists for ergonomic literal usage in tests and hand-built fixtures.
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Repeated POS labels reuse the same interned backing storage.
    ///
    /// Pointer equality here protects the memory-sharing contract of `PosCategory`.
    #[test]
    fn test_pos_category_interning() {
        let verb1 = PosCategory::new("verb");
        let verb2 = PosCategory::new("verb");

        // Same Arc (pointer equality) - strings should be interned
        assert!(Arc::ptr_eq(&verb1.0, &verb2.0));
        assert_eq!(verb1.as_str(), "verb");
        assert_eq!(verb2.as_str(), "verb");
    }

    /// Repeated stems reuse the same interned backing storage.
    ///
    /// This confirms the stem interner behaves consistently for high-frequency lemmas.
    #[test]
    fn test_mor_stem_interning() {
        let the1 = MorStem::new("the");
        let the2 = MorStem::new("the");

        // "the" is pre-populated in stem interner - should use same Arc
        assert!(Arc::ptr_eq(&the1.0, &the2.0));
        assert_eq!(the1.as_str(), "the");
    }

    /// Distinct POS labels must not alias the same interned pointer.
    ///
    /// Avoiding aliasing keeps semantic comparisons and diagnostics trustworthy.
    #[test]
    fn test_different_values_different_arcs() {
        let noun = PosCategory::new("noun");
        let verb = PosCategory::new("verb");

        // Different values - different Arcs
        assert!(!Arc::ptr_eq(&noun.0, &verb.0));
        assert_eq!(noun.as_str(), "noun");
        assert_eq!(verb.as_str(), "verb");
    }

    // =========================================================================
    // MorFeature tests
    // =========================================================================

    /// Plain features parse as flat values with no key component.
    ///
    /// This is the common `%mor` case for tags like `Plur` and `Past`.
    #[test]
    fn test_flat_feature() {
        let f = MorFeature::new("Plur");
        assert!(f.is_flat());
        assert_eq!(f.key(), None);
        assert_eq!(f.value(), "Plur");
        assert!(!f.is_empty());
    }

    /// `key=value` text parses into keyed feature shape.
    ///
    /// The test checks both key extraction and value preservation.
    #[test]
    fn test_keyed_feature() {
        let f = MorFeature::new("Number=Plur");
        assert!(!f.is_flat());
        assert_eq!(f.key(), Some("Number"));
        assert_eq!(f.value(), "Plur");
    }

    /// `flat()` constructs keyless features without parsing heuristics.
    ///
    /// This is useful for callers that already know feature shape.
    #[test]
    fn test_flat_constructor() {
        let f = MorFeature::flat("Past");
        assert!(f.is_flat());
        assert_eq!(f.value(), "Past");
    }

    /// `with_key_value()` stores keyed features exactly as provided.
    ///
    /// The constructor should not rewrite or normalize either side.
    #[test]
    fn test_with_key_value_constructor() {
        let f = MorFeature::with_key_value("Tense", "Past");
        assert!(!f.is_flat());
        assert_eq!(f.key(), Some("Tense"));
        assert_eq!(f.value(), "Past");
    }

    /// Comma-separated values remain a single lexical feature value.
    ///
    /// No decomposition is performed at this model layer.
    #[test]
    fn test_compound_feature_value() {
        // Comma-separated values like "Int,Rel" stay as a single value
        let f = MorFeature::new("Int,Rel");
        assert!(f.is_flat());
        assert_eq!(f.value(), "Int,Rel");
    }

    /// Empty feature text is representable in the model.
    ///
    /// Validation layers can decide later whether empty features are allowed.
    #[test]
    fn test_empty_feature() {
        let f = MorFeature::new("");
        assert!(f.is_empty());
        assert!(f.is_flat());
    }

    /// Flat features round-trip through serde as plain JSON strings.
    ///
    /// This guards wire-format compatibility for downstream tooling.
    #[test]
    fn test_serde_roundtrip_flat() {
        let f = MorFeature::new("Plur");
        let json = serde_json::to_string(&f).unwrap();
        assert_eq!(json, "\"Plur\"");
        let f2: MorFeature = serde_json::from_str(&json).unwrap();
        assert_eq!(f, f2);
    }

    /// Keyed features round-trip through serde as `key=value` strings.
    ///
    /// The test ensures key/value shape survives serialization boundaries.
    #[test]
    fn test_serde_roundtrip_keyed() {
        let f = MorFeature::new("Number=Plur");
        let json = serde_json::to_string(&f).unwrap();
        assert_eq!(json, "\"Number=Plur\"");
        let f2: MorFeature = serde_json::from_str(&json).unwrap();
        assert_eq!(f, f2);
    }

    /// Display for flat features emits only the raw value.
    ///
    /// This must match CHAT `%mor` textual conventions.
    #[test]
    fn test_display_flat() {
        let f = MorFeature::new("Plur");
        assert_eq!(f.to_string(), "Plur");
    }

    /// Display for keyed features emits `key=value`.
    ///
    /// The formatting path should match `WriteChat` output.
    #[test]
    fn test_display_keyed() {
        let f = MorFeature::new("Number=Plur");
        assert_eq!(f.to_string(), "Number=Plur");
    }

    /// `From<&str>` delegates to the same parser as `MorFeature::new`.
    ///
    /// This keeps literal-based construction behavior consistent.
    #[test]
    fn test_from_str() {
        let f: MorFeature = "Past".into();
        assert!(f.is_flat());
        assert_eq!(f.value(), "Past");
    }

    /// `From<String>` preserves keyed parsing behavior.
    ///
    /// Owned-string callers should get identical semantics to borrowed input.
    #[test]
    fn test_from_string() {
        let f: MorFeature = String::from("Tense=Past").into();
        assert_eq!(f.key(), Some("Tense"));
        assert_eq!(f.value(), "Past");
    }

    /// Equal feature values reuse interned value storage.
    ///
    /// Interning is important for memory usage in large `%mor` corpora.
    #[test]
    fn test_interning() {
        let f1 = MorFeature::new("Plur");
        let f2 = MorFeature::new("Plur");
        // Values should be interned (same Arc)
        assert!(Arc::ptr_eq(&f1.value, &f2.value));
    }
}
