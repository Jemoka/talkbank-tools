//! Closed vocabularies used by specific CHAT headers.
//!
//! These enums model header payloads with known variants plus an `Unsupported`
//! fallback for values the parser encounters but this implementation does not
//! recognize. The parser always succeeds; the *validator* flags unsupported
//! values.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#ID_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Media_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Number_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Recording_Quality_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Transcription_Header>

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift, ValidationTagged};

/// `@ID` sex field values.
#[derive(Debug, Clone, PartialEq, Eq, Hash, SemanticEq, SpanShift, ValidationTagged)]
pub enum Sex {
    /// `male`
    Male,
    /// `female`
    Female,
    /// Unrecognized value preserved for validation.
    Unsupported(String),
}

impl Sex {
    /// Maps canonical CHAT token text to a typed sex value.
    ///
    /// Unknown tokens yield `Unsupported` so the validator can flag them.
    pub fn from_text(value: &str) -> Self {
        match value {
            "male" => Self::Male,
            "female" => Self::Female,
            _ => Self::Unsupported(value.to_string()),
        }
    }

    /// Returns the canonical CHAT token for this sex value.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Male => "male",
            Self::Female => "female",
            Self::Unsupported(s) => s.as_str(),
        }
    }
}

impl Serialize for Sex {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Sex {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Self::from_text(&s))
    }
}

impl JsonSchema for Sex {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Sex".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({ "type": "string" })
    }
}

// ---------------------------------------------------------------------------
// RecordingQuality
// ---------------------------------------------------------------------------

/// `@Recording Quality` ratings.
#[derive(Debug, Clone, PartialEq, Eq, Hash, SemanticEq, SpanShift, ValidationTagged)]
pub enum RecordingQuality {
    /// Level 1.
    Quality1,
    /// Level 2.
    Quality2,
    /// Level 3.
    Quality3,
    /// Level 4.
    Quality4,
    /// Level 5.
    Quality5,
    /// Unrecognized value preserved for validation.
    Unsupported(String),
}

impl RecordingQuality {
    /// Maps canonical CHAT token text to a typed quality level.
    ///
    /// Unknown tokens yield `Unsupported` so the validator can flag them.
    pub fn from_text(value: &str) -> Self {
        match value {
            "1" => Self::Quality1,
            "2" => Self::Quality2,
            "3" => Self::Quality3,
            "4" => Self::Quality4,
            "5" => Self::Quality5,
            _ => Self::Unsupported(value.to_string()),
        }
    }

    /// Returns the canonical CHAT token for this quality level.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Quality1 => "1",
            Self::Quality2 => "2",
            Self::Quality3 => "3",
            Self::Quality4 => "4",
            Self::Quality5 => "5",
            Self::Unsupported(s) => s.as_str(),
        }
    }
}

impl Serialize for RecordingQuality {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for RecordingQuality {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Self::from_text(&s))
    }
}

impl JsonSchema for RecordingQuality {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "RecordingQuality".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({ "type": "string" })
    }
}

// ---------------------------------------------------------------------------
// Transcription
// ---------------------------------------------------------------------------

/// `@Transcription` values.
#[derive(Debug, Clone, PartialEq, Eq, Hash, SemanticEq, SpanShift, ValidationTagged)]
pub enum Transcription {
    /// `eye_dialect`
    EyeDialect,
    /// `partial`
    Partial,
    /// `full`
    Full,
    /// `detailed`
    Detailed,
    /// `coarse`
    Coarse,
    /// `checked`
    Checked,
    /// `anonymized`
    Anonymized,
    /// Unrecognized value preserved for validation.
    Unsupported(String),
}

impl Transcription {
    /// Maps canonical CHAT token text to a typed transcription level.
    ///
    /// Unknown tokens yield `Unsupported` so the validator can flag them.
    pub fn from_text(value: &str) -> Self {
        match value {
            "eye_dialect" => Self::EyeDialect,
            "partial" => Self::Partial,
            "full" => Self::Full,
            "detailed" => Self::Detailed,
            "coarse" => Self::Coarse,
            "checked" => Self::Checked,
            "anonymized" => Self::Anonymized,
            _ => Self::Unsupported(value.to_string()),
        }
    }

    /// Returns the canonical CHAT token for this transcription type.
    pub fn as_str(&self) -> &str {
        match self {
            Self::EyeDialect => "eye_dialect",
            Self::Partial => "partial",
            Self::Full => "full",
            Self::Detailed => "detailed",
            Self::Coarse => "coarse",
            Self::Checked => "checked",
            Self::Anonymized => "anonymized",
            Self::Unsupported(s) => s.as_str(),
        }
    }
}

impl Serialize for Transcription {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Transcription {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Self::from_text(&s))
    }
}

impl JsonSchema for Transcription {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Transcription".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({ "type": "string" })
    }
}

// ---------------------------------------------------------------------------
// MediaStatus
// ---------------------------------------------------------------------------

/// Optional third token in `@Media` describing the link/transcription state.
///
/// These statuses describe the **relationship between the transcript and the
/// media**, NOT whether the media file exists on disk. A file marked
/// `unlinked` almost certainly has media available — the transcriber just
/// hasn't aligned (bulletted) the utterances to timestamps yet.
///
/// # CHAT Manual Reference
///
/// <https://talkbank.org/0info/manuals/CHAT.html#Media_Header>
#[derive(Debug, Clone, PartialEq, Eq, Hash, SemanticEq, SpanShift, ValidationTagged)]
pub enum MediaStatus {
    /// `missing` — the media file is known to be absent or lost.
    /// Processing commands that need audio (align, transcribe) should
    /// skip this file with a clear diagnostic.
    Missing,
    /// `unlinked` — the media file EXISTS but utterances have not been
    /// aligned to timestamps yet (no bullets / time marks).
    /// This is the NORMAL state for a transcript before forced alignment.
    /// Processing commands SHOULD resolve and use the media — the whole
    /// point of `align` is to create the links that are currently absent.
    Unlinked,
    /// `notrans` — the media exists but no transcription has been done.
    /// The file may contain only headers and `@Comment` lines.
    Notrans,
    /// Unrecognized value preserved for validation.
    Unsupported(String),
}

impl MediaStatus {
    /// Maps canonical CHAT token text to a typed media status.
    ///
    /// Unknown tokens yield `Unsupported` so the validator can flag them.
    pub fn from_text(value: &str) -> Self {
        match value {
            "missing" => Self::Missing,
            "unlinked" => Self::Unlinked,
            "notrans" => Self::Notrans,
            _ => Self::Unsupported(value.to_string()),
        }
    }

    /// Returns the canonical CHAT token for this media status.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Missing => "missing",
            Self::Unlinked => "unlinked",
            Self::Notrans => "notrans",
            Self::Unsupported(s) => s.as_str(),
        }
    }
}

impl super::WriteChat for MediaStatus {
    /// Writes the CHAT token used in `@Media`.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_str(self.as_str())
    }
}

impl Serialize for MediaStatus {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for MediaStatus {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Self::from_text(&s))
    }
}

impl JsonSchema for MediaStatus {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "MediaStatus".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({ "type": "string" })
    }
}

// ---------------------------------------------------------------------------
// MediaType
// ---------------------------------------------------------------------------

/// Required second token in `@Media` describing capture modality.
#[derive(Debug, Clone, PartialEq, Eq, Hash, SemanticEq, SpanShift, ValidationTagged)]
pub enum MediaType {
    /// `audio`
    Audio,
    /// `video`
    Video,
    /// `missing` — media file itself is absent.
    Missing,
    /// Unrecognized value preserved for validation.
    Unsupported(String),
}

impl MediaType {
    /// Maps canonical CHAT token text to a typed media type.
    ///
    /// Unknown tokens yield `Unsupported` so the validator can flag them.
    pub fn from_text(value: &str) -> Self {
        match value {
            "audio" => Self::Audio,
            "video" => Self::Video,
            "missing" => Self::Missing,
            _ => Self::Unsupported(value.to_string()),
        }
    }

    /// Returns the canonical CHAT token for this media type.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Audio => "audio",
            Self::Video => "video",
            Self::Missing => "missing",
            Self::Unsupported(s) => s.as_str(),
        }
    }
}

impl super::WriteChat for MediaType {
    /// Writes the CHAT token used in `@Media`.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_str(self.as_str())
    }
}

impl Serialize for MediaType {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for MediaType {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Self::from_text(&s))
    }
}

impl JsonSchema for MediaType {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "MediaType".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({ "type": "string" })
    }
}

// ---------------------------------------------------------------------------
// Number
// ---------------------------------------------------------------------------

/// `@Number` values describing participant count scope.
#[derive(Debug, Clone, PartialEq, Eq, Hash, SemanticEq, SpanShift, ValidationTagged)]
pub enum Number {
    /// `1`
    Number1,
    /// `2`
    Number2,
    /// `3`
    Number3,
    /// `4`
    Number4,
    /// `5`
    Number5,
    /// `more`
    More,
    /// `audience`
    Audience,
    /// Unrecognized value preserved for validation.
    Unsupported(String),
}

impl Number {
    /// Maps canonical CHAT token text to a typed number value.
    ///
    /// Unknown tokens yield `Unsupported` so the validator can flag them.
    pub fn from_text(value: &str) -> Self {
        match value {
            "1" => Self::Number1,
            "2" => Self::Number2,
            "3" => Self::Number3,
            "4" => Self::Number4,
            "5" => Self::Number5,
            "more" => Self::More,
            "audience" => Self::Audience,
            _ => Self::Unsupported(value.to_string()),
        }
    }

    /// Returns the canonical CHAT token for this number value.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Number1 => "1",
            Self::Number2 => "2",
            Self::Number3 => "3",
            Self::Number4 => "4",
            Self::Number5 => "5",
            Self::More => "more",
            Self::Audience => "audience",
            Self::Unsupported(s) => s.as_str(),
        }
    }
}

impl Serialize for Number {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Number {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Self::from_text(&s))
    }
}

impl JsonSchema for Number {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Number".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({ "type": "string" })
    }
}
