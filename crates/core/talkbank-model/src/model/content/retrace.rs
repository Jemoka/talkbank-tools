//! Retrace content — words the speaker said then corrected.
//!
//! CHAT uses retrace markers (`[/]`, `[//]`, `[///]`, `[/-]`) to
//! indicate that the preceding content was spoken but then corrected or
//! repeated. Retrace content is excluded from %mor alignment but included
//! in %pho/%sin/%wor (because the material was phonologically produced).
//!
//! # CHAT Format Examples
//!
//! ```text
//! *CHI: <I want> [/] I need cookie .     ← group retrace: "I want" corrected to "I need"
//! *CHI: the [/] the dog .                ← word retrace: "the" repeated
//! *CHI: I [//] he wants it .             ← full retrace: "I" corrected to "he"
//! *CHI: <I want> [///] [//] I need .     ← multiple retrace
//! *CHI: <I go> [/-] I want to go .       ← reformulation: different phrasing
//! ```
//!
//! # Type Safety
//!
//! `Retrace` is a first-class `UtteranceContent` variant. This ensures that
//! every `match` on content must explicitly handle retraces — the compiler
//! prevents the bug class where retrace content is accidentally recursed
//! into during word extraction or retokenization.
//!
//! # References
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Retracing_Scope>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Retracing_Scope>

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

use crate::Span;
use crate::model::annotation::ContentAnnotation;
use crate::model::content::bracketed::BracketedContent;

/// Content that was spoken but then corrected/repeated.
///
/// The type-level distinction from `AnnotatedGroup` ensures that alignment,
/// retokenization, and word extraction can branch on the enum variant
/// instead of inspecting annotation contents at runtime.
///
/// # Ownership
///
/// - `content` — the retraced words (what the speaker said before correcting)
/// - `kind` — what kind of retrace (repetition, correction, reformulation)
/// - `annotations` — additional non-retrace annotations that follow the
///   retrace marker (e.g., `[= explanation]` after `[/]`)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct Retrace {
    /// The retraced content (words the speaker corrected).
    pub content: BracketedContent,
    /// What kind of retrace (repetition, correction, reformulation, etc.)
    pub kind: RetraceKind,
    /// Whether the original CHAT had angle brackets around the content.
    /// `<word> [/]` = true, `word [/]` = false.
    /// Used for lossless roundtrip serialization.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_group: bool,
    /// Additional non-retrace annotations (e.g., `[= explanation]` after `[/]`).
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub annotations: Vec<ContentAnnotation>,
    /// Source span for diagnostics.
    #[serde(skip)]
    #[schemars(skip)]
    #[semantic_eq(skip)]
    pub span: Span,
}

impl Retrace {
    /// Create a new retrace with the given content and kind.
    pub fn new(content: BracketedContent, kind: RetraceKind) -> Self {
        Self {
            content,
            kind,
            is_group: false,
            annotations: Vec::new(),
            span: Span::default(),
        }
    }

    /// Mark as originally having angle brackets (`<content> [/]`).
    pub fn as_group(mut self) -> Self {
        self.is_group = true;
        self
    }

    /// Add non-retrace annotations.
    pub fn with_annotations(mut self, annotations: Vec<ContentAnnotation>) -> Self {
        self.annotations = annotations;
        self
    }

    /// Set source span.
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = span;
        self
    }
}

/// Exhaustive retrace classification.
///
/// Adding a variant here is a compile error at every `match` site on
/// `RetraceKind` — the type system enforces handling all retrace types.
///
/// # References
///
/// - [Retracing](https://talkbank.org/0info/manuals/CHAT.html#Retracing_Scope)
/// - [Correction](https://talkbank.org/0info/manuals/CHAT.html#Retracing_Scope)
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, SemanticEq,
)]
#[serde(rename_all = "lowercase")]
pub enum RetraceKind {
    /// `[/]` — partial repetition (speaker repeats part of what they said)
    Partial,
    /// `[//]` — full retracing/correction (speaker restarts with different words)
    Full,
    /// `[///]` — multiple retracing (multiple false starts)
    Multiple,
    /// `[/-]` — reformulation (speaker rephrases with different structure)
    Reformulation,
}

impl std::fmt::Display for RetraceKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Partial => write!(f, "[/]"),
            Self::Full => write!(f, "[//]"),
            Self::Multiple => write!(f, "[///]"),
            Self::Reformulation => write!(f, "[/-]"),
        }
    }
}

impl crate::SpanShift for RetraceKind {
    fn shift_spans_after(&mut self, _threshold: u32, _delta: i32) {
        // No span fields — nothing to shift
    }
}

impl crate::model::WriteChat for RetraceKind {
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        write!(w, "{self}")
    }
}

impl crate::model::WriteChat for Retrace {
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        if self.is_group {
            w.write_char('<')?;
        }
        self.content.write_chat(w)?;
        if self.is_group {
            w.write_char('>')?;
        }
        // Write retrace marker
        w.write_char(' ')?;
        self.kind.write_chat(w)?;
        // Write additional annotations
        for ann in &self.annotations {
            w.write_char(' ')?;
            ann.write_chat(w)?;
        }
        Ok(())
    }
}

use crate::ErrorSink;
use crate::validation::{Validate, ValidationContext};

impl Validate for Retrace {
    fn validate(&self, _context: &ValidationContext, _errors: &impl ErrorSink) {
        // Retrace content validation is performed at the utterance level
        // by the cross-utterance retrace checker.
    }
}

impl Validate for RetraceKind {
    fn validate(&self, _context: &ValidationContext, _errors: &impl ErrorSink) {
        // Marker-level validation is performed at retrace-structure validation time.
    }
}
