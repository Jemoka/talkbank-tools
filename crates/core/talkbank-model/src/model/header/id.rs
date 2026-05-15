//! Typed model for the `@ID` header.
//!
//! CHAT format:
//! `@ID:\tlang|corpus|speaker|age|sex|group|ses|role|education|custom|`
//!
//! Reference:
//! - <https://talkbank.org/0info/manuals/CHAT.html#ID_Header>

use super::{
    AgeValue, CorpusName, CustomIdField, EducationDescription, GroupName, LanguageCode,
    LanguageCodes, ParticipantRole, SesValue, Sex, SpeakerCode, WriteChat,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// Parsed payload of one `@ID` header line.
///
/// This type keeps the ten-field pipe layout explicit. Optional slots are
/// serialized as empty segments so canonical output still includes the trailing
/// pipe expected by CHAT tooling.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct IDHeader {
    /// Language codes for this participant (same format as `@Languages`).
    /// May contain multiple comma-separated codes for multilingual speakers.
    pub language: LanguageCodes,

    /// Optional corpus label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub corpus: Option<CorpusName>,

    /// Required speaker code used by main-tier lines.
    pub speaker: SpeakerCode,

    /// Optional age (`years;months.days` in CHAT form).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub age: Option<AgeValue>,

    /// Optional sex token (`male` or `female`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sex: Option<Sex>,

    /// Optional group label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<GroupName>,

    /// Optional socioeconomic-status value (ethnicity, SES code, or both).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ses: Option<SesValue>,

    /// Required participant role (for example `Target_Child`, `Mother`).
    pub role: ParticipantRole,

    /// Optional education description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub education: Option<EducationDescription>,

    /// Optional corpus-specific extension field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_field: Option<CustomIdField>,
}

impl IDHeader {
    /// Builds an `@ID` payload from the required CHAT fields.
    ///
    /// For convenience, accepts a single language code string and wraps it.
    /// For multi-language IDs, use [`with_languages`](Self::with_languages).
    pub fn new(
        language: impl Into<LanguageCode>,
        speaker: impl Into<SpeakerCode>,
        role: impl Into<ParticipantRole>,
    ) -> Self {
        Self {
            language: LanguageCodes::new(vec![language.into()]),
            corpus: None,
            speaker: speaker.into(),
            age: None,
            sex: None,
            group: None,
            ses: None,
            role: role.into(),
            education: None,
            custom_field: None,
        }
    }

    /// Build from a `LanguageCodes` vec (for multi-language participants).
    pub fn from_languages(
        languages: LanguageCodes,
        speaker: impl Into<SpeakerCode>,
        role: impl Into<ParticipantRole>,
    ) -> Self {
        Self {
            language: languages,
            corpus: None,
            speaker: speaker.into(),
            age: None,
            sex: None,
            group: None,
            ses: None,
            role: role.into(),
            education: None,
            custom_field: None,
        }
    }

    /// Sets the optional corpus field.
    pub fn with_corpus(mut self, corpus: impl Into<CorpusName>) -> Self {
        self.corpus = Some(corpus.into());
        self
    }

    /// Sets the optional age field.
    pub fn with_age(mut self, age: impl Into<AgeValue>) -> Self {
        self.age = Some(age.into());
        self
    }

    /// Sets the optional sex field.
    pub fn with_sex(mut self, sex: Sex) -> Self {
        self.sex = Some(sex);
        self
    }

    /// Sets the optional group field.
    pub fn with_group(mut self, group: impl Into<GroupName>) -> Self {
        self.group = Some(group.into());
        self
    }

    /// Sets the optional SES field.
    pub fn with_ses(mut self, ses: impl Into<SesValue>) -> Self {
        self.ses = Some(ses.into());
        self
    }

    /// Sets the optional education field.
    pub fn with_education(mut self, education: impl Into<EducationDescription>) -> Self {
        self.education = Some(education.into());
        self
    }

    /// Sets the optional custom extension field.
    pub fn with_custom_field(mut self, custom_field: impl Into<CustomIdField>) -> Self {
        self.custom_field = Some(custom_field.into());
        self
    }
}

impl WriteChat for IDHeader {
    /// Serializes canonical `@ID` text with ten pipe-delimited slots.
    ///
    /// Optional fields are emitted as empty segments so output remains stable
    /// for tools that expect the trailing delimiter.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_str("@ID:\t")?;
        self.language.write_chat(w)?;
        w.write_char('|')?;

        if let Some(ref corpus) = self.corpus {
            w.write_str(corpus.as_str())?;
        }
        w.write_char('|')?;

        write!(w, "{}", self.speaker)?;
        w.write_char('|')?;

        if let Some(ref age) = self.age {
            w.write_str(age.as_str())?;
        }
        w.write_char('|')?;

        if let Some(ref sex) = self.sex {
            w.write_str(sex.as_str())?;
        }
        w.write_char('|')?;

        if let Some(ref group) = self.group {
            w.write_str(group.as_str())?;
        }
        w.write_char('|')?;

        if let Some(ref ses) = self.ses {
            write!(w, "{}", ses)?;
        }
        w.write_char('|')?;

        write!(w, "{}", self.role)?;
        w.write_char('|')?;

        if let Some(ref education) = self.education {
            w.write_str(education.as_str())?;
        }
        w.write_char('|')?;

        if let Some(ref custom_field) = self.custom_field {
            w.write_str(custom_field.as_str())?;
        }
        w.write_char('|')
    }
}

impl std::fmt::Display for IDHeader {
    /// Formats this header as canonical CHAT `@ID` text.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_chat(f)
    }
}
