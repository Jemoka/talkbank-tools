//! Phantom-tagged boundary values for text and JSON payloads.
//!
//! This module helps avoid "primitive obsession" by tagging values with
//! their intended semantic role. Python extraction wrappers now live at the
//! PyO3 boundary instead of inside this crate.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::marker::PhantomData;

/// A generic wrapper for data with associated semantic-role markers.
///
/// Uses `PhantomData` to distinguish between different types of data
/// that share the same underlying representation (e.g., String).
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Provenance<M, T = String> {
    /// The wrapped data value.
    pub data: T,
    #[serde(skip)]
    #[schemars(skip)]
    _marker: PhantomData<M>,
}

impl<M, T> Provenance<M, T> {
    /// Wrap a value with provenance metadata.
    pub fn new(data: T) -> Self {
        Self {
            data,
            _marker: PhantomData,
        }
    }
}

impl<M, T: AsRef<str>> Provenance<M, T> {
    /// Returns the data as a string slice.
    pub fn as_str(&self) -> &str {
        self.data.as_ref()
    }
}

impl<M, T: fmt::Display> fmt::Display for Provenance<M, T> {
    /// Formats only the wrapped payload value.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.data)
    }
}

// ---------------------------------------------------------------------------
// Marker Types (Zero-Sized Types)
// ---------------------------------------------------------------------------

// --- Semantic roles ---

/// Raw CHAT text intended for parsing.
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawChatText;

/// JSON representing a transcript build request.
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TranscriptJson;

/// JSON representing ASR words.
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Utterance_Media>
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AsrWordsJson;

/// A language identifier (e.g., "eng").
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Language_Codes>
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LanguageId;

/// An alignment domain name (e.g., "mor").
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TierDomainMarker;

/// Words that have been tokenized by an NLP tool.
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Word_Tier>
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenizedWords;

/// Morphosyntax produced by an NLP tool.
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Morphosyntax;

/// JSON received as a response from an NLP callback.
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NlpResponseJson;

/// Tokenized words produced by NLP.
pub type NlpTokens = Provenance<TokenizedWords, Vec<String>>;

/// Morphosyntax JSON response from NLP.
pub type NlpResponse = Provenance<NlpResponseJson, String>;
