//! Supported `@Options` tokens with parser-facing semantics.
//!
//! `@Options` values are parsed into [`ChatOptionFlag`] so downstream code can
//! branch on behavior (`CA` parsing rules, bullet handling) without ad hoc
//! string checks. Unrecognized values are stored as `Unsupported(String)` so
//! the validator can flag them.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Options_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#CA_Option>
//! - <https://talkbank.org/0info/manuals/CHAT.html#CA_Unicode_Option>

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift, ValidationTagged};

#[derive(Clone, Debug, PartialEq, Eq, SemanticEq, SpanShift, ValidationTagged)]
/// `@Options` tokens with behavior in this implementation.
///
/// Known flags carry parser-facing semantics (CA mode, alignment skip).
/// Unrecognized values are preserved for validation but do not affect parsing.
pub enum ChatOptionFlag {
    /// `CA`: enable Conversation Analysis mode.
    Ca,
    /// `NoAlign`: skip forced alignment for this file.
    NoAlign,
    /// Unrecognized value preserved for validation.
    Unsupported(String),
}

impl ChatOptionFlag {
    /// Maps canonical CHAT token text to a typed option flag.
    ///
    /// Unknown tokens yield `Unsupported` so the validator can flag them.
    pub fn from_text(value: &str) -> Self {
        match value {
            "CA" => Self::Ca,
            "NoAlign" => Self::NoAlign,
            _ => Self::Unsupported(value.to_string()),
        }
    }

    /// Returns the canonical token emitted when serializing this flag.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Ca => "CA",
            Self::NoAlign => "NoAlign",
            Self::Unsupported(s) => s.as_str(),
        }
    }

    /// Returns `true` when this flag turns on Conversation Analysis parsing rules.
    pub fn enables_ca_mode(&self) -> bool {
        matches!(self, Self::Ca)
    }

    /// Returns `true` when this flag indicates forced alignment should be skipped.
    pub fn skips_alignment(&self) -> bool {
        matches!(self, Self::NoAlign)
    }
}

impl Serialize for ChatOptionFlag {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ChatOptionFlag {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Self::from_text(&s))
    }
}

impl JsonSchema for ChatOptionFlag {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "ChatOptionFlag".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({ "type": "string" })
    }
}
