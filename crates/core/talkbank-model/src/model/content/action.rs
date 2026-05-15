//! Model for the CHAT action marker (`0`).
//!
//! This token records a non-verbal action at a word position.
//!
//! # CHAT Format References
//!
//! - [Action Code](https://talkbank.org/0info/manuals/CHAT.html#Action_Code)

use super::WriteChat;
use crate::Span;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// Non-verbal action token represented as `0`.
///
/// This is distinct from omitted-word forms (`0word`), which are modeled via
/// [`crate::model::WordCategory::Omission`].
///
/// # CHAT Format Examples
///
/// ```text
/// *CHI: 0 .              Child performs action without speaking
/// *MOT: what did you do 0 ?
/// ```
///
/// # Important Distinction
///
/// - `0` alone = Action (this struct)
/// - `0word` = Omitted word (see [`crate::model::WordCategory::Omission`])
///
/// # References
///
/// - [Action Code](https://talkbank.org/0info/manuals/CHAT.html#Action_Code)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct Action {
    /// Source location metadata for diagnostics.
    #[serde(skip)]
    #[schemars(skip)]
    #[semantic_eq(skip)]
    pub span: Span,
}

impl Action {
    /// Build an action token with dummy span metadata.
    ///
    /// Parser fixtures and hand-built model values typically use this, then
    /// optionally attach real span data later in parser-owned paths.
    pub fn new() -> Self {
        Self { span: Span::DUMMY }
    }

    /// Build an action token with explicit source span metadata.
    ///
    /// Use this constructor when source offsets are already known at creation.
    pub fn with_span(span: Span) -> Self {
        Self { span }
    }
}

impl Default for Action {
    /// Returns an action token with dummy span metadata.
    fn default() -> Self {
        Self::new()
    }
}

impl WriteChat for Action {
    /// Serializes the canonical action token (`0`).
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_char('0')
    }
}

impl std::fmt::Display for Action {
    /// Formats the canonical CHAT surface form (`0`).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_chat(f)
    }
}
