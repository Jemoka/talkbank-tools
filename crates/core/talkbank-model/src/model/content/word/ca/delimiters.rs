//! Conversation-analysis (CA) paired prosodic delimiters used inside words.
//!
//! CHAT reference anchors:
//! - [CA Delimiters](https://talkbank.org/0info/manuals/CHAT.html#CA_Delimiters)
//! - [CA Subwords](https://talkbank.org/0info/manuals/CHAT.html#CA_Subwords)

use crate::model::WriteChat;
use crate::validation::{Validate, ValidationContext};
use crate::{ErrorSink, Span};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// Paired CA prosodic delimiter type.
///
/// Symbol mapping is defined by [`CADelimiterType::to_symbol`] and must stay
/// aligned with the parser grammar.
///
/// # Variants
///
/// **Speech Rate:**
/// - `Faster` (∆) - Faster speech
/// - `Slower` (∇) - Slower speech
///
/// **Volume:**
/// - `Softer` (°)
/// - `Louder` (◉)
///
/// **Pitch Range:**
/// - `LowPitch` (▁)
/// - `HighPitch` (▔)
///
/// **Voice Quality:**
/// - `SmileVoice` (☺) - Smile voice/laughter quality
/// - `BreathyVoice` (♋) - Breathy voice quality
/// - `Whisper` (∬) - Whispered speech
/// - `Creaky` (⁎) - Creaky voice
/// - `Yawn` (Ϋ) - Yawning quality
/// - `Singing` (∮) - Singing voice
///
/// **Other:**
/// - `Unsure` (⁇)
/// - `SegmentRepetition` (↫) - Segment repetition
/// - `Precise` (§) - Precise articulation
///
/// # CHAT Format Examples
///
/// ```text
/// ∆fast∆             # Faster
/// ∇slow∇             # Slower
/// °soft°             # Softer
/// ☺smile☺            # SmileVoice
/// ∬whisper∬          # Whisper
/// ```
///
/// # References
///
/// - [Words](https://talkbank.org/0info/manuals/CHAT.html#Words)
/// - [Annotations](https://talkbank.org/0info/manuals/CHAT.html#Annotations)
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
    SemanticEq,
    SpanShift,
)]
#[serde(rename_all = "snake_case")]
pub enum CADelimiterType {
    /// `∆`
    Faster,
    /// `∇`
    Slower,
    /// `°`
    Softer,
    /// `▁`
    LowPitch,
    /// `▔`
    HighPitch,
    /// `☺`
    SmileVoice,
    /// `♋`
    BreathyVoice,
    /// `⁇`
    Unsure,
    /// `∬`
    Whisper,
    /// `Ϋ`
    Yawn,
    /// `∮`
    Singing,
    /// `↫`
    SegmentRepetition,
    /// `⁎`
    Creaky,
    /// `◉`
    Louder,
    /// `§`
    Precise,
}

impl CADelimiterType {
    /// Returns the CHAT symbol for this CA delimiter type.
    ///
    /// Symbols must remain synchronized with `tree-sitter-talkbank` token
    /// definitions so parsing and rendering are inverse-compatible.
    pub fn to_symbol(&self) -> &'static str {
        match self {
            CADelimiterType::Faster => "∆",            // U+2206 INCREMENT
            CADelimiterType::Slower => "∇",            // U+2207 NABLA
            CADelimiterType::Softer => "°",            // U+00B0 DEGREE SIGN
            CADelimiterType::LowPitch => "▁",          // U+2581 LOWER ONE EIGHTH BLOCK
            CADelimiterType::HighPitch => "▔",         // U+2594 UPPER ONE EIGHTH BLOCK
            CADelimiterType::SmileVoice => "☺",        // U+263A WHITE SMILING FACE
            CADelimiterType::BreathyVoice => "♋",     // U+264B CANCER
            CADelimiterType::Unsure => "⁇",            // U+2047 DOUBLE QUESTION MARK
            CADelimiterType::Whisper => "∬",           // U+222C DOUBLE INTEGRAL
            CADelimiterType::Yawn => "Ϋ", // U+03AB GREEK CAPITAL UPSILON WITH DIALYTIKA
            CADelimiterType::Singing => "∮", // U+222E CONTOUR INTEGRAL
            CADelimiterType::SegmentRepetition => "↫", // U+21AB LEFTWARDS ARROW WITH LOOP
            CADelimiterType::Creaky => "⁎", // U+204E LOW ASTERISK
            CADelimiterType::Louder => "◉", // U+25C9 FISHEYE
            CADelimiterType::Precise => "§", // U+00A7 SECTION SIGN
        }
    }
}

/// One CA delimiter token used to bound a prosodic region.
///
/// # Structure
///
/// A CA delimiter consists of:
/// - **type**: The kind of prosodic modification (rate, volume, voice quality)
/// - **span**: Optional source location information
///
/// # CHAT Format Examples
///
/// ```text
/// *CHI: I want ∆that∆ .                  # Faster speech
/// *MOT: °okay° .                         # Softer speech
/// *CHI: ∬thank you∬ .                    # Smile voice
/// *INV: ∇very slow∇ .                    # Slower speech
/// ```
///
/// # Delimiter Pairing
///
/// CA delimiters should be balanced within an utterance:
/// ```text
/// ∆fast∆             # ✅ Balanced
/// ∆fast              # ❌ Unbalanced - validation error E230
/// °soft∆             # ❌ Mismatched - validation error E230
/// ```
///
/// # Usage
///
/// ```rust
/// use talkbank_model::{CADelimiter, CADelimiterType};
///
/// let faster = CADelimiter::new(CADelimiterType::Faster);
/// let softer = CADelimiter::new(CADelimiterType::Softer);
/// ```
///
/// # References
///
/// - [Words](https://talkbank.org/0info/manuals/CHAT.html#Words)
/// - [Annotations](https://talkbank.org/0info/manuals/CHAT.html#Annotations)
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
    SemanticEq,
    SpanShift,
)]
pub struct CADelimiter {
    /// Delimiter variant.
    pub delimiter_type: CADelimiterType,
    /// Optional source location metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[semantic_eq(skip)]
    pub span: Option<Span>,
}

impl CADelimiter {
    /// Build a CA delimiter token with no span metadata.
    ///
    /// Parser paths generally call this first and attach spans later if source
    /// tracking is available.
    pub fn new(delimiter_type: CADelimiterType) -> Self {
        Self {
            delimiter_type,
            span: None,
        }
    }

    /// Attach source span metadata.
    ///
    /// Spans are optional and only affect diagnostics, never semantic equality.
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }
}

impl WriteChat for CADelimiter {
    /// Writes the exact Unicode symbol for this CA delimiter.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_str(self.delimiter_type.to_symbol())
    }
}

impl Validate for CADelimiter {
    /// Pairing/balance constraints are validated at utterance-level CA checks.
    ///
    /// Single delimiter tokens are structurally valid on their own; only
    /// cross-token pairing state determines delimiter errors.
    fn validate(&self, _context: &ValidationContext, _errors: &impl ErrorSink) {}
}
