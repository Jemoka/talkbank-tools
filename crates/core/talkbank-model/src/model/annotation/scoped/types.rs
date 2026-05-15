//! Scoped annotation model types (`[*]`, `[=]`, retracing, overlaps, and related markers).
//!
//! These types capture the parser's normalized representation of CHAT scoped
//! symbols so validation and serialization can operate on a closed enum instead
//! of stringly marker handling.
//!

use crate::validation::{Validate, ValidationContext};
use crate::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// Scoped annotation that modifies or provides information about speech content.
///
/// Scoped annotations in CHAT format are enclosed in square brackets and provide
/// contextual information about errors, clarifications, overlaps, and repetitions.
///
/// # Annotation Types
///
/// - **Error marking** (`[*]`, `[* code]`): Indicates speech errors, grammatical mistakes,
///   or phonological errors that need correction or special attention.
///
/// - **Explanations** (`[= text]`): Clarifies unintelligible speech, unusual pronunciations,
///   or ambiguous utterances. Often used with `xxx` for unintelligible material.
///
/// - **Retracing** (`[/]`, `[//]`, `[///]`): Marks self-corrections and repeated words.
///   Single `/` for partial repetition, double for full retracing, triple for multiple.
///
/// - **Overlaps** (`[<]`, `[>]`): Marks simultaneous speech by different speakers.
///   `[<]` at overlap start, `[>]` at overlap end.
///
/// # CHAT Manual Reference
///
/// - [Error Coding](https://talkbank.org/0info/manuals/CHAT.html#Error_Coding)
/// - [Explanation Scope](https://talkbank.org/0info/manuals/CHAT.html#Explanation_Scope)
/// - [Retracing](https://talkbank.org/0info/manuals/CHAT.html#Retracing_and_Repetition)
/// - [Overlap Precedes Scope](https://talkbank.org/0info/manuals/CHAT.html#OverlapPrecedes_Scope)
/// - [Overlap Follows Scope](https://talkbank.org/0info/manuals/CHAT.html#OverlapFollows_Scope)
///
/// # Examples
///
/// ```
/// use talkbank_model::model::{ContentAnnotation, ScopedError, ScopedExplanation};
///
/// // Error marking
/// let error = ContentAnnotation::Error(ScopedError { code: Some("grammar".into()) });
///
/// // Explanation
/// let explanation = ContentAnnotation::Explanation(ScopedExplanation {
///     text: "probably said ball".into()
/// });
/// ```
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ContentAnnotation {
    /// Error marking (`[*]` or `[* code]`).
    ///
    /// Marks speech errors, grammatical mistakes, or phonological errors.
    /// Optional error code specifies the type of error (e.g., "grammar", "phonology").
    ///
    /// **Examples:**
    /// - `[*]` - Generic error marker
    /// - `[* grammar]` - Grammatical error
    /// - `[* phonology]` - Phonological error
    ///
    /// See: [Error Coding](https://talkbank.org/0info/manuals/CHAT.html#Error_Coding)
    Error(ScopedError),

    /// Explanation (`[= text]`).
    ///
    /// Clarifies unclear or unintelligible speech. Commonly used with `xxx` to explain
    /// what was likely said when the actual utterance is unintelligible.
    ///
    /// **Examples:**
    /// - `xxx [= probably ball]`
    /// - `doggie [= referring to cat]`
    ///
    /// See: [Explanation Scope](https://talkbank.org/0info/manuals/CHAT.html#Explanation_Scope)
    Explanation(ScopedExplanation),

    /// Overlap beginning marker (`[<]`).
    ///
    /// Marks the point where simultaneous speech begins. Used when two or more
    /// speakers talk at the same time.
    ///
    /// **Example:**
    /// ```text
    /// *CHI: I want [<] that .
    /// *MOT: you want [>] what ?
    /// ```
    ///
    /// See: [Overlap Precedes Scope](https://talkbank.org/0info/manuals/CHAT.html#OverlapPrecedes_Scope)
    #[serde(rename = "overlap_begin")]
    OverlapBegin(ScopedOverlapBegin),

    /// Overlap ending marker (`[>]`).
    ///
    /// Marks the point where simultaneous speech ends.
    ///
    /// See: [Overlap Follows Scope](https://talkbank.org/0info/manuals/CHAT.html#OverlapFollows_Scope)
    #[serde(rename = "overlap_end")]
    OverlapEnd(ScopedOverlapEnd),

    /// Scoped stressing marker (`[!]`).
    ///
    /// Marks emphatic stress or emphasis on preceding word/phrase.
    ///
    /// **Example:** `that [!]` - emphatic stress
    ///
    /// See: [Scoped Symbols](https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols)
    Stressing,

    /// Scoped contrastive stressing (`[!!]`).
    ///
    /// Marks strong contrastive stress.
    ///
    /// **Example:** `mine [!!]` - strong contrastive stress
    ///
    /// See: [Scoped Symbols](https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols)
    ContrastiveStressing,

    /// Scoped uncertain (`[?]`).
    ///
    /// Marks uncertain or unclear transcription.
    ///
    /// **Example:** `doggie [?]` - uncertain transcription
    ///
    /// See: [Scoped Symbols](https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols)
    Uncertain,

    /// Paralinguistic annotation (`[=! text]`).
    ///
    /// Describes paralinguistic features like whispering, laughing, etc.
    ///
    /// **Example:** `hello [=! whispers]`
    ///
    /// See: [Paralinguistic Material Scope](https://talkbank.org/0info/manuals/CHAT.html#ParalinguisticMaterial_Scope)
    Paralinguistic(ScopedParalinguistic),

    /// Alternative transcription (`[=? text]`).
    ///
    /// Provides alternative interpretation or uncertain transcription.
    ///
    /// **Example:** `xxx [=? maybe ball]`
    ///
    /// See: [Alternative Transcription Scope](https://talkbank.org/0info/manuals/CHAT.html#AlternativeTranscription_Scope)
    Alternative(ScopedAlternative),

    /// Percent annotation (`[% text]`).
    ///
    /// General comment or note about the utterance.
    ///
    /// **Example:** `hey [% comment about context]`
    ///
    /// See: [Comment Scope](https://talkbank.org/0info/manuals/CHAT.html#Comment_Scope)
    PercentComment(ScopedPercentComment),

    /// Exclude marker (`[e]`).
    ///
    /// Marks content to be excluded from analysis.
    ///
    /// See: [Excluded Material](https://talkbank.org/0info/manuals/CHAT.html#MorExclude_Scope)
    Exclude,

    /// Unknown annotation (lenient parsing).
    ///
    /// Captures annotations with unrecognized markers. This allows the parser
    /// to accept all CHAT files while flagging unusual annotations for review.
    Unknown(ScopedUnknown),
}

/// Error marking data for `[*]` or `[* code]` annotations.
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Error_Coding>
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
pub struct ScopedError {
    /// Optional error type code
    pub code: Option<smol_str::SmolStr>,
}

/// Explanation data for `[= text]` annotations.
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Explanation_Scope>
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
pub struct ScopedExplanation {
    /// Explanatory text
    pub text: smol_str::SmolStr,
}

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
/// Numeric index (1-9) for distinguishing multiple overlaps in a single utterance.
///
/// References:
/// - <https://talkbank.org/0info/manuals/CHAT.html#OverlapPrecedes_Scope>
/// - <https://talkbank.org/0info/manuals/CHAT.html#OverlapFollows_Scope>
#[serde(transparent)]
pub struct OverlapMarkerIndex(pub u8);

impl OverlapMarkerIndex {
    /// Create an overlap marker index from a digit payload.
    ///
    /// Range validation (`1..=9`) is performed by [`Validate`] so parser paths
    /// can construct first and report context-rich diagnostics later.
    pub fn new(index: u8) -> Self {
        Self(index)
    }
}

impl std::fmt::Display for OverlapMarkerIndex {
    /// Formats the stored overlap index digit (`1`-`9`).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Validate for OverlapMarkerIndex {
    /// Enforces CHAT overlap-index range constraints (single digit `1` through `9`).
    fn validate(&self, context: &ValidationContext, errors: &impl ErrorSink) {
        if (1..=9).contains(&self.0) {
            return;
        }

        let index_str = self.0.to_string();
        let span = match context.field_span {
            Some(span) => span,
            None => Span::from_usize(0, index_str.len()),
        };
        let location = match context.field_span {
            Some(span) => SourceLocation::new(span),
            None => SourceLocation::at_offset(0),
        };
        let source_text = match context.field_text.clone() {
            Some(text) => text,
            None => index_str.clone(),
        };
        // DEFAULT: Missing label falls back to "overlap_marker_index" for error messaging.
        let label = context.field_label.unwrap_or("overlap_marker_index");

        errors.report(
            ParseError::new(
                ErrorCode::InvalidOverlapIndex,
                Severity::Error,
                location,
                ErrorContext::new(source_text, span, label),
                format!("Overlap marker index {} is invalid", self.0),
            )
            .with_suggestion("Overlap marker indices must be a single digit from 1 to 9"),
        );
    }
}

/// Overlap begin marker data for `[<]` or `[<N]` annotations.
///
/// # Reference
///
/// - [Overlap precedes scope](https://talkbank.org/0info/manuals/CHAT.html#OverlapPrecedes_Scope)
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
pub struct ScopedOverlapBegin {
    /// Optional index for multiple overlaps (`[<1]`, `[<2]`, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<OverlapMarkerIndex>,
}

/// Overlap end marker data for `[>]` or `[>N]` annotations.
///
/// # Reference
///
/// - [Overlap follows scope](https://talkbank.org/0info/manuals/CHAT.html#OverlapFollows_Scope)
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
pub struct ScopedOverlapEnd {
    /// Optional index for multiple overlaps (`[>1]`, `[>2]`, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<OverlapMarkerIndex>,
}

/// Paralinguistic annotation data for `[=! text]`.
///
/// # Reference
///
/// - [Paralinguistic material scope](https://talkbank.org/0info/manuals/CHAT.html#ParalinguisticMaterial_Scope)
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
pub struct ScopedParalinguistic {
    /// Description of paralinguistic feature
    pub text: smol_str::SmolStr,
}

/// Alternative transcription data for `[=? text]`.
///
/// # Reference
///
/// - [Alternative transcription scope](https://talkbank.org/0info/manuals/CHAT.html#AlternativeTranscription_Scope)
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
pub struct ScopedAlternative {
    /// Alternative transcription text
    pub text: smol_str::SmolStr,
}

/// Percent comment data for `[% text]` annotations.
///
/// # Reference
///
/// - [Comment scope](https://talkbank.org/0info/manuals/CHAT.html#Comment_Scope)
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
pub struct ScopedPercentComment {
    /// Comment text
    pub text: smol_str::SmolStr,
}

/// Unknown annotation captured during lenient parsing.
///
/// # Reference
///
/// - [Scoped symbols](https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols)
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
pub struct ScopedUnknown {
    /// The annotation marker (e.g., custom markers)
    pub marker: smol_str::SmolStr,
    /// The annotation text
    pub text: smol_str::SmolStr,
}
