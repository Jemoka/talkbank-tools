//! Postcode tokens (`[+ ...]`) used for utterance-level analysis tags.
//!
//! # CHAT Format References
//!
//! - [Postcodes](https://talkbank.org/0info/manuals/CHAT.html#Postcodes)

use super::WriteChat;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// Utterance-level postcode payload.
///
/// The model stores raw postcode text and leaves interpretation to downstream
/// tooling/pipelines.
///
/// # CHAT Format Examples
///
/// ```text
/// *CHI: I want cookie [+ exc] .      Exclude from analysis
/// *MOT: very good [+ imp] !          Imitation prompt
/// ```
///
/// # Common Postcodes
///
/// - `[+ exc]` - Exclude utterance
/// - `[+ trn]` - Translation
/// - `[+ gram]` - Grammatical
/// - `[+ jar]` - Jargon
///
/// # References
///
/// - [Postcodes](https://talkbank.org/0info/manuals/CHAT.html#Postcodes)
/// - [Excluded Utterance Postcode](https://talkbank.org/0info/manuals/CHAT.html#ExcludedUtterancePostcode)
/// - [Included Utterance Postcode](https://talkbank.org/0info/manuals/CHAT.html#IncludedUtterancePostcode)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct Postcode {
    /// Payload text written after `[+ ` and before `]`.
    pub text: smol_str::SmolStr,

    /// Source location metadata for diagnostics (not serialized).
    #[serde(skip)]
    #[schemars(skip)]
    pub span: crate::Span,
}

impl Postcode {
    /// Build a postcode with dummy span metadata.
    ///
    /// This is the default constructor for synthetic fixtures and transformed
    /// model values where source offsets are not preserved.
    pub fn new(text: impl Into<smol_str::SmolStr>) -> Self {
        Self {
            text: text.into(),
            span: crate::Span::DUMMY,
        }
    }

    /// Attach source span metadata used for diagnostics.
    ///
    /// Span values are ignored by semantic-equality comparisons.
    pub fn with_span(mut self, span: crate::Span) -> Self {
        self.span = span;
        self
    }
}

impl WriteChat for Postcode {
    /// Serializes canonical CHAT postcode syntax (`[+ ...]`).
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_str("[+ ")?;
        w.write_str(&self.text)?;
        w.write_char(']')
    }
}

impl std::fmt::Display for Postcode {
    /// Formats the postcode as CHAT text (`[+ <text>]`).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_chat(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression: simple postcode text roundtrips through `Display`.
    #[test]
    fn postcode_simple_roundtrip() {
        let postcode = Postcode::new("bch");
        let output = postcode.to_string();
        assert_eq!(output, "[+ bch]", "Postcode roundtrip failed");
    }

    /// Regression: postcode text with spaces is preserved during serialization.
    #[test]
    fn postcode_with_spaces_roundtrip() {
        let postcode = Postcode::new("long text with spaces");
        let output = postcode.to_string();
        assert_eq!(
            output, "[+ long text with spaces]",
            "Postcode with spaces roundtrip failed"
        );
    }
}
