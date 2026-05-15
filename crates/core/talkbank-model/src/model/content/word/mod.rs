//! Word-level CHAT content models.
//!
//! This subtree contains the typed building blocks for lexical tokens on the
//! main tier, including internal prosody markers and language/form suffixes.
//! The `Word` type in `types.rs` composes these pieces into one canonical unit.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Word_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#SecondLanguage_Marker>
//! - <https://talkbank.org/0info/manuals/CHAT.html#PrimaryStress_Element>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Lengthening_Marker>
//! - <https://talkbank.org/0info/manuals/CHAT.html#WordInternalPause_Marker>

/// Conversation Analysis (CA) prosodic markers within words.
pub mod ca;
/// Word category prefixes (omission, filler, fragment, nonword).
pub mod category;
/// Internal word content elements (text, shortening, stress, lengthening, etc.).
pub mod content;
/// Special form markers (`@a`..`@z:label`).
pub mod form;
/// Language override markers (`@s`, `@s:code`).
pub mod language;
/// Untranscribed word status (`xxx`, `yyy`, `www`).
pub mod untranscribed;
/// [`WordContents`] — ordered sequence of content elements.
mod word_contents;
/// Serialization and display implementations for [`Word`].
mod word_serialize;
/// Core [`Word`] struct and methods.
mod word_type;
/// [`Validate`] implementations for [`Word`] and [`WordContents`].
mod word_validate;

pub use ca::{CADelimiter, CADelimiterType, CAElement, CAElementType};
pub use category::WordCategory;
pub use content::{
    UnderlineMarker, WordCliticBoundary, WordCompoundMarker, WordContent, WordLengthening,
    WordShortening, WordStressMarker, WordStressMarkerType, WordSyllablePause, WordText,
    WordUnderlineBegin, WordUnderlineEnd,
};
pub use form::FormType;
pub use language::WordLanguageMarker;
pub use untranscribed::UntranscribedStatus;
pub use word_contents::WordContents;
pub use word_type::Word;
