//! Utterance-level language metadata (tier default + per-word resolution).
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Language_Codes>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Language_Switching>

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::ops::{Deref, DerefMut};
use talkbank_derive::SpanShift;

use super::super::LanguageCode;
use super::{WordLanguageInfo, WordLanguages};
use crate::ErrorSink;
use crate::validation::{Validate, ValidationContext};

/// Language metadata for an entire utterance.
///
/// Tracks the resolved language for each alignable word in the main tier.
/// Parallel to alignment metadata - indexed by alignable content position.
///
/// This structure stores:
/// - **tier language**: utterance baseline (`[- code]` or `@Languages` primary)
/// - **word languages**: resolved per-word language/provenance entries
///
/// The `word_languages` vector is indexed the same way as alignment pairs,
/// making it easy to correlate language information with aligned tier data.
///
/// # Structure
///
/// ```text
/// LanguageMetadata
///   ├─ tier_language: Option<LanguageCode>  (e.g., "eng")
///   └─ word_languages: Vec<WordLanguageInfo>
///        ├─ [0]: word_index=0, language="eng", source=Default
///        ├─ [1]: word_index=1, language="spa", source=WordShortcut
///        └─ ...
/// ```
///
/// # CHAT Format Examples
///
/// **Example 1: Single language utterance**
///
/// ```text
/// @Languages: eng
/// *CHI: I want cookie .
/// ```
///
/// Language metadata:
/// - `tier_language`: Some("eng")
/// - `word_languages`: All words have language="eng", source=Default
/// - `is_code_switching()`: false
///
/// **Example 2: Code-switching utterance**
///
/// ```text
/// @Languages: eng, spa
/// *CHI: I want @s galletas @s please .
/// ```
///
/// Language metadata:
/// - `tier_language`: Some("eng")
/// - `word_languages`:
///   - "I" → eng (Default)
///   - "want" → eng (Default)
///   - "galletas" → spa (WordShortcut)
///   - "please" → eng (WordShortcut)
/// - `is_code_switching()`: true (uses both "eng" and "spa")
///
/// **Example 3: Tier-scoped language change**
///
/// ```text
/// @Languages: eng, fra
/// *CHI: [- fra] je veux cookie .
/// ```
///
/// Language metadata:
/// - `tier_language`: Some("fra") (from `[- fra]` marker)
/// - `word_languages`: All words have language="fra", source=TierScoped
/// - `is_code_switching()`: false
///
/// # Use Cases
///
/// **Code-switching detection:**
/// ```rust,ignore
/// if language_metadata.is_code_switching() {
///     let counts = language_metadata.count_by_language();
///     println!("Utterance uses {} languages", counts.len());
/// }
/// ```
///
/// **Per-word language lookup:**
/// ```rust,ignore
/// if let Some(info) = language_metadata.get_word_language(2) {
///     println!("Word 2 is in language: {:?}", info.language);
/// }
/// ```
///
/// # References
///
/// - [Language Codes](https://talkbank.org/0info/manuals/CHAT.html#Language_Codes)
/// - [Language Switching](https://talkbank.org/0info/manuals/CHAT.html#Language_Switching)
/// - [Dependent Tiers](https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SpanShift)]
pub struct LanguageMetadata {
    /// Effective utterance baseline language.
    ///
    /// Applies to words without explicit language markers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tier_language: Option<LanguageCode>,

    /// Resolved language/provenance for each alignable word.
    ///
    /// Indexing is aligned with the utterance alignable-word domain.
    pub word_languages: WordLanguageInfos,
}

impl LanguageMetadata {
    /// Create metadata with a baseline tier language and no word entries.
    ///
    /// This constructor is typically used before word-level resolution begins.
    /// Word assignments are appended incrementally as alignable content is traversed.
    pub fn new(tier_language: Option<LanguageCode>) -> Self {
        Self {
            tier_language,
            word_languages: WordLanguageInfos::new(Vec::new()),
        }
    }

    /// Append one resolved word-language entry.
    ///
    /// Callers should add entries in alignable-word order so `word_index`
    /// remains comparable with alignment metadata (`%mor/%gra/%pho/%wor`).
    pub fn add_word(&mut self, info: WordLanguageInfo) {
        self.word_languages.0.push(info);
    }

    /// Look up language metadata by alignable word index.
    ///
    /// This performs an index-keyed search over stored entries and returns the
    /// first match. Missing entries indicate unresolved or unrecorded language state.
    pub fn get_word_language(&self, index: usize) -> Option<&WordLanguageInfo> {
        self.word_languages.iter().find(|w| w.word_index == index)
    }

    /// Count word assignments by language code.
    ///
    /// For single-language words, increments the count for that language.
    /// For code-mixed or ambiguous words, increments counts for ALL applicable languages.
    pub fn count_by_language(&self) -> HashMap<LanguageCode, usize> {
        let mut counts = HashMap::new();
        for word_info in self.word_languages.iter() {
            // Count all applicable languages, even for code-mixed or ambiguous words
            for lang in word_info.languages.languages() {
                *counts.entry(lang.clone()).or_insert(0) += 1;
            }
        }
        counts
    }

    /// Return whether this utterance should be treated as code-switching.
    ///
    /// This returns `true` when at least two distinct languages appear across
    /// words, or when any word is explicitly marked as mixed (`@s:eng+spa`) or
    /// ambiguous (`@s:eng&spa`), even if distinct-language counts collapse.
    pub fn is_code_switching(&self) -> bool {
        let mut languages: HashSet<_> = HashSet::new();
        let mut has_multiple = false;
        let mut has_ambiguous = false;

        for word_info in self.word_languages.iter() {
            match &word_info.languages {
                WordLanguages::Single(lang) => {
                    languages.insert(lang.clone());
                }
                WordLanguages::Multiple(_) => {
                    has_multiple = true;
                    // Also collect individual languages
                    for lang in word_info.languages.languages() {
                        languages.insert(lang.clone());
                    }
                }
                WordLanguages::Ambiguous(_) => {
                    has_ambiguous = true;
                    // Also collect individual languages
                    for lang in word_info.languages.languages() {
                        languages.insert(lang.clone());
                    }
                }
                WordLanguages::Unresolved => {}
            }
        }

        // Code-switching if multiple distinct languages are present, or any
        // per-word assignment is explicitly multiple/ambiguous.
        languages.len() > 1 || has_multiple || has_ambiguous
    }
}

/// Newtype wrapper around per-word language resolution entries for one utterance.
///
/// The vector order follows alignable-word indexing used by the parser and
/// alignment code, so `%mor`/`%gra` item positions can be compared with
/// language switching decisions word-by-word.
///
/// References:
/// - <https://talkbank.org/0info/manuals/CHAT.html#Language_Switching>
/// - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SpanShift)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct WordLanguageInfos(pub Vec<WordLanguageInfo>);

impl WordLanguageInfos {
    /// Wrap a vector of per-word language entries.
    ///
    /// This constructor is mostly used by parser/build pipelines that already
    /// assembled a full utterance-level assignment vector.
    pub fn new(infos: Vec<WordLanguageInfo>) -> Self {
        Self(infos)
    }

    /// Return whether no per-word language entries are present.
    ///
    /// Empty metadata is valid for empty/error-recovered utterances and should
    /// not be interpreted as a parser failure by itself.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Deref for WordLanguageInfos {
    type Target = Vec<WordLanguageInfo>;

    /// Expose the underlying vector for read-only iteration and indexing.
    ///
    /// The transparent wrapper keeps schema typing while still allowing normal
    /// `Vec` ergonomics in analysis code.
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for WordLanguageInfos {
    /// Expose the underlying vector for in-place mutation.
    ///
    /// Mutations should preserve `word_index` invariants expected by alignment
    /// and language-resolution consumers.
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<Vec<WordLanguageInfo>> for WordLanguageInfos {
    /// Wrap a raw vector as a typed language-info list.
    ///
    /// Prefer this conversion when bridging from parser internals to the
    /// strongly-typed model surface.
    fn from(infos: Vec<WordLanguageInfo>) -> Self {
        Self(infos)
    }
}

impl Validate for WordLanguageInfos {
    /// Word-level validation is performed where entries are produced.
    ///
    /// This container itself currently has no extra invariants beyond storing
    /// a list of `WordLanguageInfo`. The hook remains in place so future
    /// cross-entry invariants can be enforced without changing call sites.
    fn validate(&self, _context: &ValidationContext, _errors: &impl ErrorSink) {}
}

impl Default for LanguageMetadata {
    /// Create empty metadata with no tier language and no word entries.
    ///
    /// This default is used for parser-recovery and builder initialization
    /// before language resolution has run.
    fn default() -> Self {
        Self::new(None)
    }
}
