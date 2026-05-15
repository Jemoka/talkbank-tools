//! Separator tokens used in CHAT main-tier content.
//!
//! Includes punctuation-like separators and CA-specific continuation/intonation
//! markers that are modeled as standalone content items.
//!
//! # CHAT Format References
//!
//! - [Separators](https://talkbank.org/0info/manuals/CHAT.html#Separators)
//! - [Satellite Marker](https://talkbank.org/0info/manuals/CHAT.html#Satellite_Marker)
//! - [Comma](https://talkbank.org/0info/manuals/CHAT.html#Comma)
//! - [Colon](https://talkbank.org/0info/manuals/CHAT.html#Colon)

use super::WriteChat;
use crate::Span;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// Separator token variants.
///
/// This enum intentionally keeps CA-specific markers adjacent to standard
/// punctuation separators so parser and renderer logic can share one token type.
///
/// # CHAT Format Examples
///
/// **Pauses and Breaks:**
/// ```text
/// *CHI: I want, um, cookies .        # Comma - short pause within TCU
/// *MOT: I think; maybe not .         # Semicolon - longer pause
/// *CHI: that's co:ol .               # Colon - elongation/emphasis
/// ```
///
/// **CA Intonation Markers:**
/// ```text
/// *CHI: really ⇗                     # Rising to high intonation
/// *MOT: maybe ↗                      # Rising to mid intonation
/// *CHI: I know →                     # Level/continuing intonation
/// *MOT: okay ↘                       # Falling to mid intonation
/// *CHI: done ⇘                       # Falling to low intonation
/// ```
///
/// **Special CA Markers:**
/// ```text
/// *CHI: you know„                    # TAG marker (question tag)
/// *MOT: John‡ come here .            # VOCATIVE marker (addressing)
/// *CHI: I think [^c]                 # CA continuation (ongoing turn)
/// *MOT: so∞                          # Unmarked ending (no clear terminal intonation)
/// *CHI: yeah≡                        # Uptake/latching (no gap between turns)
/// *MOT: keep going ≈                 # No-break TCU separator
/// *CHI: mic issue ≋                  # Technical-break TCU separator
/// ```
///
/// # References
///
/// - [Separators](https://talkbank.org/0info/manuals/CHAT.html#Separators)
/// - [CA Intonation](https://talkbank.org/0info/manuals/CHAT.html#CA_Intonation)
/// - [Comma](https://talkbank.org/0info/manuals/CHAT.html#Comma)
/// - [Colon](https://talkbank.org/0info/manuals/CHAT.html#Colon)
/// - [Satellite Marker](https://talkbank.org/0info/manuals/CHAT.html#Satellite_Marker)
#[derive(
    Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
#[serde(tag = "kind")]
pub enum Separator {
    /// Comma separator (,) - short pause within TCU
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Comma>
    #[serde(rename = "comma")]
    Comma {
        #[serde(skip)]
        #[schemars(skip)]
        #[semantic_eq(skip)]
        /// Source span for error reporting.
        span: Span,
    },

    /// Semicolon separator (;) - longer pause, list continuation
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Semicolon>
    #[serde(rename = "semicolon")]
    Semicolon {
        #[serde(skip)]
        #[schemars(skip)]
        #[semantic_eq(skip)]
        /// Source span for error reporting.
        span: Span,
    },

    /// Colon separator (:) - elongation or emphasis
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Colon>
    #[serde(rename = "colon")]
    Colon {
        #[serde(skip)]
        #[schemars(skip)]
        #[semantic_eq(skip)]
        /// Source span for error reporting.
        span: Span,
    },

    /// „ (U+201E) - TAG marker (double low-9 quotation mark) - Conversation Analysis
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#TAG_Marker>
    #[serde(rename = "tag")]
    Tag {
        #[serde(skip)]
        #[schemars(skip)]
        #[semantic_eq(skip)]
        /// Source span for error reporting.
        span: Span,
    },

    /// ‡ (U+2021) - VOCATIVE marker (double dagger) - Conversation Analysis
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Vocative_Marker>
    #[serde(rename = "vocative")]
    Vocative {
        #[serde(skip)]
        #[schemars(skip)]
        #[semantic_eq(skip)]
        /// Source span for error reporting.
        span: Span,
    },

    /// [^c] - CA continuation marker - Conversation Analysis
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#CA_Continuation>
    #[serde(rename = "ca_continuation")]
    CaContinuation {
        #[serde(skip)]
        #[schemars(skip)]
        #[semantic_eq(skip)]
        /// Source span for error reporting.
        span: Span,
    },

    /// ∞ (U+221E) - Unmarked ending (no clear terminal intonation) - Conversation Analysis
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#UnmarkedEnding>
    #[serde(rename = "unmarked_ending")]
    UnmarkedEnding {
        #[serde(skip)]
        #[schemars(skip)]
        #[semantic_eq(skip)]
        /// Source span for error reporting.
        span: Span,
    },

    /// ≡ (U+2261) - Uptake/latching (no gap between turns) - Conversation Analysis
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Uptake_Marker>
    #[serde(rename = "uptake")]
    Uptake {
        #[serde(skip)]
        #[schemars(skip)]
        #[semantic_eq(skip)]
        /// Source span for error reporting.
        span: Span,
    },

    /// ≈ (U+2248) - No-break TCU separator - Conversation Analysis
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#TCU_NoBreak>
    #[serde(rename = "ca_no_break")]
    CaNoBreak {
        #[serde(skip)]
        #[schemars(skip)]
        #[semantic_eq(skip)]
        /// Source span for error reporting.
        span: Span,
    },

    /// ≋ (U+224B) - Technical-break TCU separator - Conversation Analysis
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#TCU_Technical_Break>
    #[serde(rename = "ca_technical_break")]
    CaTechnicalBreak {
        #[serde(skip)]
        #[schemars(skip)]
        #[semantic_eq(skip)]
        /// Source span for error reporting.
        span: Span,
    },

    /// ⇗ (U+21D7) - Rising to high intonation - Conversation Analysis
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#RisingToHigh>
    #[serde(rename = "rising_to_high")]
    RisingToHigh {
        #[serde(skip)]
        #[schemars(skip)]
        #[semantic_eq(skip)]
        /// Source span for error reporting.
        span: Span,
    },

    /// ↗ (U+2197) - Rising to mid intonation - Conversation Analysis
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#RisingToMid>
    #[serde(rename = "rising_to_mid")]
    RisingToMid {
        #[serde(skip)]
        #[schemars(skip)]
        #[semantic_eq(skip)]
        /// Source span for error reporting.
        span: Span,
    },

    /// → (U+2192) - Level intonation (continuing) - Conversation Analysis
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Level_Intonation>
    #[serde(rename = "level")]
    Level {
        #[serde(skip)]
        #[schemars(skip)]
        #[semantic_eq(skip)]
        /// Source span for error reporting.
        span: Span,
    },

    /// ↘ (U+2198) - Falling to mid intonation - Conversation Analysis
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#FallingToMid>
    #[serde(rename = "falling_to_mid")]
    FallingToMid {
        #[serde(skip)]
        #[schemars(skip)]
        #[semantic_eq(skip)]
        /// Source span for error reporting.
        span: Span,
    },

    /// ⇘ (U+21D8) - Falling to low intonation - Conversation Analysis
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#FallingToLow>
    #[serde(rename = "falling_to_low")]
    FallingToLow {
        #[serde(skip)]
        #[schemars(skip)]
        #[semantic_eq(skip)]
        /// Source span for error reporting.
        span: Span,
    },
}

impl Separator {
    /// Returns source span metadata associated with this separator.
    pub fn span(&self) -> Span {
        match self {
            Separator::Comma { span }
            | Separator::Semicolon { span }
            | Separator::Colon { span }
            | Separator::Tag { span }
            | Separator::Vocative { span }
            | Separator::CaContinuation { span }
            | Separator::UnmarkedEnding { span }
            | Separator::Uptake { span }
            | Separator::CaNoBreak { span }
            | Separator::CaTechnicalBreak { span }
            | Separator::RisingToHigh { span }
            | Separator::RisingToMid { span }
            | Separator::Level { span }
            | Separator::FallingToMid { span }
            | Separator::FallingToLow { span } => *span,
        }
    }

    /// Returns `true` when this separator is a comma token.
    pub fn is_comma(&self) -> bool {
        matches!(self, Separator::Comma { .. })
    }
}

impl WriteChat for Separator {
    /// Serializes the canonical CHAT token for this separator.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        match self {
            Separator::Comma { .. } => w.write_str(","),
            Separator::Semicolon { .. } => w.write_str(";"),
            Separator::Colon { .. } => w.write_str(":"),
            Separator::Tag { .. } => w.write_char(crate::chars::TAG_MARKER),
            Separator::Vocative { .. } => w.write_char(crate::chars::VOCATIVE_MARKER),
            Separator::CaContinuation { .. } => w.write_str("[^c]"),
            Separator::UnmarkedEnding { .. } => w.write_str("\u{221E}"),
            Separator::Uptake { .. } => w.write_str("\u{2261}"),
            Separator::CaNoBreak { .. } => w.write_str("\u{2248}"),
            Separator::CaTechnicalBreak { .. } => w.write_str("\u{224B}"),
            Separator::RisingToHigh { .. } => w.write_str("\u{21D7}"),
            Separator::RisingToMid { .. } => w.write_str("\u{2197}"),
            Separator::Level { .. } => w.write_str("\u{2192}"),
            Separator::FallingToMid { .. } => w.write_str("\u{2198}"),
            Separator::FallingToLow { .. } => w.write_str("\u{21D8}"),
        }
    }
}

impl std::fmt::Display for Separator {
    /// Formats the separator as CHAT surface text.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_chat(f)
    }
}
