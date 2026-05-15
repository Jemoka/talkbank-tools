//! Conversation-analysis (CA) single-point prosodic markers used inside words.
//!
//! CHAT reference anchors:
//! - [CA Subwords](https://talkbank.org/0info/manuals/CHAT.html#CA_Subwords)
//! - [CA Delimiters](https://talkbank.org/0info/manuals/CHAT.html#CA_Delimiters)

use crate::model::WriteChat;
use crate::validation::{Validate, ValidationContext};
use crate::{ErrorSink, Span};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// Single-point CA prosodic marker type.
///
/// Symbol mapping is defined by [`CAElementType::to_symbol`] and must stay
/// aligned with the parser grammar.
///
/// # Variants
///
/// **Pitch Markers:**
/// - `PitchUp` (↑)
/// - `PitchDown` (↓)
/// - `PitchReset` (↻)
///
/// **Other Markers:**
/// - `BlockedSegments` (≠)
/// - `Constriction` (∾)
/// - `Hardening` (⁑)
/// - `HurriedStart` (⤇)
/// - `Inhalation` (∙)
/// - `LaughInWord` (Ἡ)
/// - `SuddenStop` (⤆)
///
/// # CHAT Format Examples
///
/// ```text
/// ↑hello             # pitch up
/// ↓there             # pitch down
/// ≠blocked           # blocked segment
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
pub enum CAElementType {
    /// `≠`
    BlockedSegments,
    /// `∾`
    Constriction,
    /// `⁑`
    Hardening,
    /// `⤇`
    HurriedStart,
    /// `∙`
    Inhalation,
    /// `Ἡ`
    LaughInWord,
    /// `↓`
    PitchDown,
    /// `↻`
    PitchReset,
    /// `↑`
    PitchUp,
    /// `⤆`
    SuddenStop,
}

impl CAElementType {
    /// Returns the CHAT symbol for this CA element type.
    ///
    /// Symbols must remain synchronized with `tree-sitter-talkbank` token
    /// definitions so parsing and rendering are inverse-compatible.
    pub fn to_symbol(&self) -> &'static str {
        match self {
            CAElementType::BlockedSegments => "≠", // U+2260 NOT EQUAL TO
            CAElementType::Constriction => "∾",    // U+223E INVERTED LAZY S
            CAElementType::Hardening => "⁑",       // U+2051 TWO ASTERISKS
            CAElementType::HurriedStart => "⤇",    // U+2907 RIGHTWARDS DOUBLE DASH ARROW
            CAElementType::Inhalation => "∙",      // U+2219 BULLET OPERATOR
            CAElementType::LaughInWord => "Ἡ",     // U+1F29 GREEK CAPITAL ETA WITH DASIA
            CAElementType::PitchDown => "↓",       // U+2193 DOWNWARDS ARROW
            CAElementType::PitchReset => "↻",      // U+21BB CLOCKWISE OPEN CIRCLE ARROW
            CAElementType::PitchUp => "↑",         // U+2191 UPWARDS ARROW
            CAElementType::SuddenStop => "⤆",      // U+2906 LEFTWARDS DOUBLE DASH ARROW
        }
    }
}

/// One concrete CA prosodic marker token.
///
/// # Structure
///
/// A CA element consists of:
/// - **type**: The kind of prosodic marker (pitch, stress, etc.)
/// - **span**: Optional source location information
///
/// # CHAT Format Examples
///
/// ```text
/// *CHI: ↑hello .                         # Pitch rise
/// *MOT: ˈvery nice .                     # Primary stress
/// *CHI: I ↓know .                        # Pitch fall
/// *INV: ≠wait .                          # Blocked segment
/// ```
///
/// # Usage
///
/// ```rust
/// use talkbank_model::{CAElement, CAElementType};
///
/// let pitch_up = CAElement::new(CAElementType::PitchUp);
/// let pitch = CAElement::new(CAElementType::PitchUp);
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
pub struct CAElement {
    /// Marker variant.
    pub element_type: CAElementType,
    /// Optional source location metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[semantic_eq(skip)]
    pub span: Option<Span>,
}

impl CAElement {
    /// Build a CA element token with no span metadata.
    ///
    /// Parser paths generally call this first and attach spans later if source
    /// tracking is available.
    pub fn new(element_type: CAElementType) -> Self {
        Self {
            element_type,
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

impl WriteChat for CAElement {
    /// Writes the exact Unicode symbol for this CA element.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_str(self.element_type.to_symbol())
    }
}

impl Validate for CAElement {
    /// Element-level constraints are validated by higher-level word structure checks.
    ///
    /// A single CA element token has no independent balance constraints, so
    /// this validator is intentionally a no-op.
    fn validate(&self, _context: &ValidationContext, _errors: &impl ErrorSink) {}
}
