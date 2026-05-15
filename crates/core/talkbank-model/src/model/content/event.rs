//! Model for CHAT event tokens (`&=...`).
//!
//! Events represent non-speech sounds/actions inline in main-tier content.
//!
//! # CHAT Format References
//!
//! - [Simple Events](https://talkbank.org/0info/manuals/CHAT.html#SimpleEvents)
//! - [Local Events](https://talkbank.org/0info/manuals/CHAT.html#Local_Event)

use super::WriteChat;
use crate::string_newtype;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

string_newtype!(
    /// Event descriptor payload written after `&=`.
    ///
    /// This is modeled as an unconstrained string newtype because corpora often
    /// introduce project-specific event labels.
    ///
    /// # CHAT Format Examples
    ///
    /// ```text
    /// &=laughs        Simple laughter event
    /// &=coughs        Coughing event
    /// &=clears:throat Throat clearing with detail
    /// &=crying        Crying event
    /// ```
    ///
    /// # References
    ///
    /// - [Simple Events](https://talkbank.org/0info/manuals/CHAT.html#SimpleEvents)
    /// - [Local Events](https://talkbank.org/0info/manuals/CHAT.html#Local_Event)
    pub struct EventType;
);

/// Inline non-speech event marker.
///
/// Serializes as `&=<event_type>`.
///
/// # CHAT Format Examples
///
/// ```text
/// *MOT: hello &=laughs there !
/// *CHI: um &=coughs I don't know .
/// ```
///
/// # References
///
/// - [Simple Events](https://talkbank.org/0info/manuals/CHAT.html#SimpleEvents)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct Event {
    /// Event descriptor token (for example `laughs`, `coughs`, `clears:throat`).
    pub event_type: EventType,

    /// Source location metadata for diagnostics (not serialized).
    #[serde(skip)]
    #[schemars(skip)]
    pub span: crate::Span,
}

impl Event {
    /// Build an event marker with dummy span metadata.
    ///
    /// This is the common constructor for parser/model assembly before source
    /// offsets are finalized.
    pub fn new(event_type: impl Into<EventType>) -> Self {
        Self {
            event_type: event_type.into(),
            span: crate::Span::DUMMY,
        }
    }

    /// Attach source span metadata used by diagnostics.
    ///
    /// Span metadata is intentionally excluded from semantic equality.
    pub fn with_span(mut self, span: crate::Span) -> Self {
        self.span = span;
        self
    }
}

impl WriteChat for Event {
    /// Serializes canonical CHAT event syntax (`&=<event_type>`).
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_str("&=")?;
        self.event_type.write_chat(w)
    }
}

impl std::fmt::Display for Event {
    /// Formats the event in CHAT form (`&=<event_type>`).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_chat(f)
    }
}
