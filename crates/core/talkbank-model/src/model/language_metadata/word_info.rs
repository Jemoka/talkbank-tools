//! Per-word language-resolution types used for CHAT language switching.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Language_Switching>
//! - <https://talkbank.org/0info/manuals/CHAT.html#SecondLanguage_Marker_Single>
//! - <https://talkbank.org/0info/manuals/CHAT.html#SecondLanguage_Marker_Multiple>
//! - <https://talkbank.org/0info/manuals/CHAT.html#SecondLanguage_Marker_Ambiguous>

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SpanShift, ValidationTagged};

use super::super::LanguageCode;
use super::LanguageSource;

/// The languages applicable to a word, preserving code-mixing and ambiguity information.
///
/// This enum captures the complete semantic information about a word's language(s):
/// - **Single**: One definitive language (explicit marker, shortcut resolved, or tier default)
/// - **Multiple**: Code-mixed - word contains content from multiple languages simultaneously (@s:eng+fra)
/// - **Ambiguous**: Ambiguous between languages - transcriber couldn't decide (@s:eng&spa)
/// - **Unresolved**: No language context is available for this word
///
/// References:
/// - <https://talkbank.org/0info/manuals/CHAT.html#Language_Switching>
/// - <https://talkbank.org/0info/manuals/CHAT.html#SecondLanguage_Marker_Single>
/// - <https://talkbank.org/0info/manuals/CHAT.html#SecondLanguage_Marker_Multiple>
/// - <https://talkbank.org/0info/manuals/CHAT.html#SecondLanguage_Marker_Ambiguous>
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SpanShift, ValidationTagged,
)]
pub enum WordLanguages {
    /// Single definitive language
    Single(LanguageCode),
    /// Multiple languages mixed together (code-mixing)
    Multiple(Vec<LanguageCode>),
    /// Ambiguous between languages
    Ambiguous(Vec<LanguageCode>),
    /// No language could be resolved for this word
    #[validation_tag(error)]
    Unresolved,
}

impl WordLanguages {
    /// Return all language codes referenced by this assignment.
    ///
    /// `Single` returns one entry; `Multiple` and `Ambiguous` return each
    /// member in source order; `Unresolved` returns an empty list.
    pub fn languages(&self) -> Vec<&LanguageCode> {
        match self {
            Self::Single(code) => vec![code],
            Self::Multiple(codes) | Self::Ambiguous(codes) => codes.iter().collect(),
            Self::Unresolved => Vec::new(),
        }
    }

    /// Return `true` for explicit code-mixed assignments.
    ///
    /// This maps to CHAT markers like `@s:eng+spa` where one token is treated
    /// as jointly belonging to multiple languages.
    pub fn is_code_mixed(&self) -> bool {
        matches!(self, Self::Multiple(_))
    }

    /// Return `true` for explicitly ambiguous assignments.
    ///
    /// This maps to CHAT markers like `@s:eng&spa` where the transcriber left
    /// uncertainty between candidate languages.
    pub fn is_ambiguous(&self) -> bool {
        matches!(self, Self::Ambiguous(_))
    }
}

/// Language metadata for a single word.
///
/// Stores the resolved language(s) and source for one alignable word
/// in the main tier. The `word_index` corresponds to the position in
/// the alignable content (same indexing used for tier alignment).
///
/// This structure is used for:
/// - **Code-switching analysis**: Identify which words are in which language(s)
/// - **Code-mixing detection**: Identify words with @s:eng+fra style markers
/// - **Ambiguity tracking**: Identify words with @s:eng&spa style markers
/// - **Validation**: Ensure language markers are used correctly
/// - **Data extraction**: Associate morphological annotations with language context
///
/// # Fields
///
/// - `word_index`: Zero-based index in alignable content (words + groups + terminator)
/// - `languages`: Resolved language(s): Single, Multiple (code-mixed), or Ambiguous
/// - `source`: How the language was determined (see [`LanguageSource`])
///
/// # CHAT Format Examples
///
/// **Example 1: Code-switching with shortcuts**
///
/// ```text
/// @Languages: eng, spa
/// *CHI: I want @s galletas @s please .
/// ```
///
/// Language metadata:
/// - Word 0 "I": languages=Single("eng"), source=Default
/// - Word 1 "want": languages=Single("eng"), source=Default
/// - Word 2 "galletas": languages=Single("spa"), source=WordShortcut
/// - Word 3 "please": languages=Single("eng"), source=WordShortcut
/// - Word 4 ".": languages=Single("eng"), source=Default
///
/// **Example 2: Code-mixed word**
///
/// ```text
/// @Languages: eng, spa
/// *CHI: hello habla@s:eng+spa .
/// ```
///
/// Language metadata:
/// - Word 0 "hello": languages=Single("eng"), source=Default
/// - Word 1 "habla": languages=Multiple(["eng", "spa"]), source=WordExplicit
/// - Word 2 ".": languages=Single("eng"), source=Default
///
/// **Example 3: Ambiguous word**
///
/// ```text
/// @Languages: eng, spa
/// *CHI: hello word@s:eng&spa .
/// ```
///
/// Language metadata:
/// - Word 0 "hello": languages=Single("eng"), source=Default
/// - Word 1 "word": languages=Ambiguous(["eng", "spa"]), source=WordExplicit
/// - Word 2 ".": languages=Single("eng"), source=Default
///
/// # References
///
/// - [Language Codes](https://talkbank.org/0info/manuals/CHAT.html#Language_Codes)
/// - [Language Switching](https://talkbank.org/0info/manuals/CHAT.html#Language_Switching)
/// - [Second-Language Marker (single)](https://talkbank.org/0info/manuals/CHAT.html#SecondLanguage_Marker_Single)
/// - [Second-Language Marker (multiple)](https://talkbank.org/0info/manuals/CHAT.html#SecondLanguage_Marker_Multiple)
/// - [Second-Language Marker (ambiguous)](https://talkbank.org/0info/manuals/CHAT.html#SecondLanguage_Marker_Ambiguous)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SpanShift)]
pub struct WordLanguageInfo {
    /// Zero-based index of the word in alignable content
    pub word_index: usize,

    /// Resolved language(s): Single, Multiple (code-mixed), or Ambiguous
    pub languages: WordLanguages,

    /// How the language was determined
    pub source: LanguageSource,
}

impl WordLanguageInfo {
    /// Construct per-word language metadata from explicit pieces.
    ///
    /// Use this when callers already resolved both language payload and source
    /// and just need to materialize the strongly-typed model value.
    pub fn new(word_index: usize, languages: WordLanguages, source: LanguageSource) -> Self {
        Self {
            word_index,
            languages,
            source,
        }
    }

    /// Construct a single-language entry sourced from `@Languages`.
    ///
    /// This is the default path for unmarked words in utterances with a known
    /// tier baseline language.
    pub fn default_language(word_index: usize, language: impl Into<LanguageCode>) -> Self {
        Self {
            word_index,
            languages: WordLanguages::Single(language.into()),
            source: LanguageSource::Default,
        }
    }

    /// Construct a single-language entry sourced from `[- code]`.
    ///
    /// Use this when utterance-level scope markers override file-level defaults.
    pub fn tier_scoped(word_index: usize, language: impl Into<LanguageCode>) -> Self {
        Self {
            word_index,
            languages: WordLanguages::Single(language.into()),
            source: LanguageSource::TierScoped,
        }
    }

    /// Construct a single-language entry sourced from explicit word marker.
    ///
    /// This captures `@s:code` cases where one language is named directly on the token.
    pub fn word_explicit(word_index: usize, language: impl Into<LanguageCode>) -> Self {
        Self {
            word_index,
            languages: WordLanguages::Single(language.into()),
            source: LanguageSource::WordExplicit,
        }
    }

    /// Construct a code-mixed entry from explicit marker.
    ///
    /// This captures explicit multi-language tokens such as `@s:eng+spa`.
    pub fn word_explicit_multiple(word_index: usize, languages: Vec<LanguageCode>) -> Self {
        Self {
            word_index,
            languages: WordLanguages::Multiple(languages),
            source: LanguageSource::WordExplicit,
        }
    }

    /// Construct an ambiguous-language entry from explicit marker.
    ///
    /// This captures explicit ambiguous markers such as `@s:eng&spa`.
    pub fn word_explicit_ambiguous(word_index: usize, languages: Vec<LanguageCode>) -> Self {
        Self {
            word_index,
            languages: WordLanguages::Ambiguous(languages),
            source: LanguageSource::WordExplicit,
        }
    }

    /// Construct a single-language entry sourced from `@s` shortcut.
    ///
    /// The language value should be the post-toggle resolved target, not the
    /// currently active tier language.
    pub fn word_shortcut(word_index: usize, language: impl Into<LanguageCode>) -> Self {
        Self {
            word_index,
            languages: WordLanguages::Single(language.into()),
            source: LanguageSource::WordShortcut,
        }
    }

    /// Construct an unresolved language entry.
    ///
    /// Use this when language could not be inferred safely and callers should
    /// avoid fabricating fallback semantics.
    pub fn unresolved(word_index: usize) -> Self {
        Self {
            word_index,
            languages: WordLanguages::Unresolved,
            source: LanguageSource::Unresolved,
        }
    }
}
