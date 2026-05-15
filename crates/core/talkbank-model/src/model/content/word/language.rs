//! Word-level language override markers for code-switching (`@s` forms).
//!
//! CHAT reference anchors:
//! - [Languages header](https://talkbank.org/0info/manuals/CHAT.html#Languages_Header)
//! - [Language switching](https://talkbank.org/0info/manuals/CHAT.html#Language_Switching)

use crate::model::LanguageCode;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use talkbank_derive::{SemanticEq, SpanShift};

/// Word-level language override marker (`@s` forms).
///
/// Used to represent explicit per-word language choice when it differs from the
/// utterance-level/default language context.
///
/// # CHAT Format Examples
///
/// ```text
/// istenem@s:hu      Word in Hungarian (explicit code)
/// dog@s             Toggle to secondary language (shortcut)
/// gracias@s:spa     Word in Spanish
/// merci@s:fra       Word in French
/// hello@s:eng+fra   Word in multiple languages (code-mixed)
/// word@s:eng&spa    Ambiguous between English and Spanish
/// ```
///
/// # Forms
///
/// - **Shortcut** (`@s`) - Toggles between primary and secondary languages declared in headers
/// - **Explicit** (`@s:code`) - Specifies exact language using ISO 639-3 code
/// - **Multiple** (`@s:code+code+...`) - Word uses multiple languages mixed together
/// - **Ambiguous** (`@s:code&code&...`) - Word could be in any of the listed languages
///
/// # References
///
/// - [Languages header](https://talkbank.org/0info/manuals/CHAT.html#Languages_Header)
/// - [Language switching](https://talkbank.org/0info/manuals/CHAT.html#Language_Switching)
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
#[serde(tag = "type", content = "code", rename_all = "snake_case")]
pub enum WordLanguageMarker {
    /// Bare `@s` shortcut toggling between primary and secondary languages.
    Shortcut,
    /// Explicit language code (`@s:code`) using ISO 639-3.
    Explicit(LanguageCode),
    /// Multiple languages mixed together (`@s:eng+fra`).
    Multiple(Vec<LanguageCode>),
    /// Ambiguous between multiple languages (`@s:eng&spa`).
    Ambiguous(Vec<LanguageCode>),
}

impl WordLanguageMarker {
    /// Build an explicit single-language override (`@s:code`).
    ///
    /// Use this when the transcript names one concrete language code on the word.
    pub fn explicit(lang: impl Into<LanguageCode>) -> Self {
        Self::Explicit(lang.into())
    }

    /// Build a code-mixed override (`@s:code+code+...`).
    ///
    /// This represents simultaneous multi-language content within one token.
    pub fn multiple(langs: Vec<LanguageCode>) -> Self {
        Self::Multiple(langs)
    }

    /// Build an ambiguous override (`@s:code&code&...`).
    ///
    /// This represents uncertainty between alternatives rather than code mixing.
    pub fn ambiguous(langs: Vec<LanguageCode>) -> Self {
        Self::Ambiguous(langs)
    }

    /// Returns primary language code when one can be inferred.
    ///
    /// For multi-code variants, this returns the first listed code. This helper
    /// is convenience-oriented and should not replace full multi-code handling.
    pub fn as_language(&self) -> Option<&LanguageCode> {
        match self {
            WordLanguageMarker::Shortcut => None,
            WordLanguageMarker::Explicit(code) => Some(code),
            WordLanguageMarker::Multiple(codes) => codes.first(),
            WordLanguageMarker::Ambiguous(codes) => codes.first(),
        }
    }

    /// Return `true` if this is the bare `@s` shortcut form.
    ///
    /// Shortcut resolution depends on broader language context and cannot be
    /// interpreted in isolation.
    pub fn is_shortcut(&self) -> bool {
        matches!(self, WordLanguageMarker::Shortcut)
    }

    /// Return all codes for code-mixed (`+`) form.
    ///
    /// This returns `None` for non-`Multiple` variants to avoid silent coercion.
    pub fn as_multiple(&self) -> Option<&[LanguageCode]> {
        match self {
            WordLanguageMarker::Multiple(codes) => Some(codes),
            _ => None,
        }
    }

    /// Return all codes for ambiguous (`&`) form.
    ///
    /// Ambiguous lists should be treated as alternatives, not as jointly active
    /// language membership.
    pub fn as_ambiguous(&self) -> Option<&[LanguageCode]> {
        match self {
            WordLanguageMarker::Ambiguous(codes) => Some(codes),
            _ => None,
        }
    }
}
