//! Typed representation of CHAT header lines and header-list wrapper types.
//!
//! The parser stores transcript order at the file layer (`Line::Header`), while
//! this module captures the structured payload of each header for validation,
//! transformation, and serialization.
//!
//! CHAT reference anchors:
//! - [File headers](https://talkbank.org/0info/manuals/CHAT.html#File_Headers)
//! - [Participants header](https://talkbank.org/0info/manuals/CHAT.html#Participants_Header)
//! - [Languages header](https://talkbank.org/0info/manuals/CHAT.html#Languages_Header)
//! - [Options header](https://talkbank.org/0info/manuals/CHAT.html#Options_Header)

use super::super::{
    codes::{
        ActivitiesDescription, BackgroundDescription, BirthplaceDescription, ChatDate,
        ColorWordList, FontSpec, GemLabel, LanguageCode, LanguageName, LocationDescription,
        PageNumber, ParticipantEntry, PidValue, RoomLayoutDescription, SituationDescription,
        SpeakerCode, TDescription, TapeLocationDescription, TimeDurationValue, TimeStartValue,
        TranscriberName, VideoSpec, WarningText, WindowGeometry,
    },
    enums::{Number, RecordingQuality, Transcription},
    types_header::TypesHeader,
};
use super::options::ChatOptionFlag;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};
use talkbank_derive::{SemanticEq, SpanShift};

/// Ordered payload of an `@Languages` header.
///
/// Order is semantically relevant: the first code is treated as the transcript
/// default when utterances do not override language explicitly.
///
/// # Reference
///
/// - [Languages header](https://talkbank.org/0info/manuals/CHAT.html#Languages_Header)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct LanguageCodes(pub Vec<LanguageCode>);

impl LanguageCodes {
    /// Wraps an already parsed `@Languages` sequence.
    pub fn new(codes: Vec<LanguageCode>) -> Self {
        Self(codes)
    }

    /// Returns `true` when no language was declared.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl crate::model::WriteChat for LanguageCodes {
    /// Writes comma-separated language codes: `eng, spa`.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        for (i, code) in self.0.iter().enumerate() {
            if i > 0 {
                w.write_str(", ")?;
            }
            code.write_chat(w)?;
        }
        Ok(())
    }
}

impl Deref for LanguageCodes {
    type Target = Vec<LanguageCode>;

    /// Borrows the underlying ordered language-code list.
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for LanguageCodes {
    /// Mutably borrows the underlying language-code list.
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<Vec<LanguageCode>> for LanguageCodes {
    /// Wraps an owned vector without copying.
    fn from(codes: Vec<LanguageCode>) -> Self {
        Self(codes)
    }
}

impl<'a> IntoIterator for &'a LanguageCodes {
    type Item = &'a LanguageCode;
    type IntoIter = std::slice::Iter<'a, LanguageCode>;

    /// Iterates immutably over language codes in header order.
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'a> IntoIterator for &'a mut LanguageCodes {
    type Item = &'a mut LanguageCode;
    type IntoIter = std::slice::IterMut<'a, LanguageCode>;

    /// Iterates mutably over language codes.
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut()
    }
}

impl IntoIterator for LanguageCodes {
    type Item = LanguageCode;
    type IntoIter = std::vec::IntoIter<LanguageCode>;

    /// Consumes the wrapper and yields owned language codes.
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl Default for LanguageCodes {
    /// Returns an empty language-code list (no `@Languages` header).
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl crate::validation::Validate for LanguageCodes {
    /// Enforces non-empty `@Languages` and validates each language code entry.
    fn validate(
        &self,
        context: &crate::validation::ValidationContext,
        errors: &impl crate::ErrorSink,
    ) {
        if self.is_empty() {
            errors.report(crate::ParseError::new(
                crate::ErrorCode::EmptyLanguagesHeader,
                crate::Severity::Error,
                crate::SourceLocation::at_offset(0),
                crate::ErrorContext::new("", 0..0, "languages"),
                "@Languages header should specify at least one language code",
            ));
        }

        for code in &self.0 {
            code.validate(context, errors);
        }
    }
}

/// Ordered payload of an `@Participants` header.
///
/// Order is preserved from source so tools can roundtrip the author's header
/// layout and keep participant listings stable in UIs.
///
/// # Reference
///
/// - [Participants header](https://talkbank.org/0info/manuals/CHAT.html#Participants_Header)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct ParticipantEntries(pub Vec<ParticipantEntry>);

impl ParticipantEntries {
    /// Wraps parsed participant entries in source order.
    pub fn new(entries: Vec<ParticipantEntry>) -> Self {
        Self(entries)
    }

    /// Returns `true` when no participants were declared.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Deref for ParticipantEntries {
    type Target = Vec<ParticipantEntry>;

    /// Borrows the underlying ordered participant-entry list.
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ParticipantEntries {
    /// Mutably borrows the underlying participant-entry list.
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<Vec<ParticipantEntry>> for ParticipantEntries {
    /// Wraps an owned vector without copying.
    fn from(entries: Vec<ParticipantEntry>) -> Self {
        Self(entries)
    }
}

impl<'a> IntoIterator for &'a ParticipantEntries {
    type Item = &'a ParticipantEntry;
    type IntoIter = std::slice::Iter<'a, ParticipantEntry>;

    /// Iterates immutably over participant entries.
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'a> IntoIterator for &'a mut ParticipantEntries {
    type Item = &'a mut ParticipantEntry;
    type IntoIter = std::slice::IterMut<'a, ParticipantEntry>;

    /// Iterates mutably over participant entries.
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut()
    }
}

impl IntoIterator for ParticipantEntries {
    type Item = ParticipantEntry;
    type IntoIter = std::vec::IntoIter<ParticipantEntry>;

    /// Consumes the wrapper and yields owned participant entries.
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl crate::validation::Validate for ParticipantEntries {
    /// Enforces non-empty `@Participants` and validates each declared participant entry.
    fn validate(
        &self,
        context: &crate::validation::ValidationContext,
        errors: &impl crate::ErrorSink,
    ) {
        if self.is_empty() {
            errors.report(crate::ParseError::new(
                crate::ErrorCode::EmptyParticipantsHeader,
                crate::Severity::Error,
                crate::SourceLocation::at_offset(0),
                crate::ErrorContext::new("", 0..0, "participants"),
                "@Participants header cannot be empty",
            ));
        }

        for entry in &self.0 {
            entry.validate(context, errors);
        }
    }
}

/// Ordered payload of an `@Options` header.
///
/// Flags are retained in source order even though most consumers treat this as
/// a set, which keeps roundtrip output deterministic.
///
/// # Reference
///
/// - [Options header](https://talkbank.org/0info/manuals/CHAT.html#Options_Header)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct ChatOptionFlags(pub Vec<ChatOptionFlag>);

impl ChatOptionFlags {
    /// Wraps parsed option flags in source order.
    pub fn new(options: Vec<ChatOptionFlag>) -> Self {
        Self(options)
    }

    /// Returns `true` when no options were declared.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Deref for ChatOptionFlags {
    type Target = Vec<ChatOptionFlag>;

    /// Borrows the underlying ordered option-flag list.
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ChatOptionFlags {
    /// Mutably borrows the underlying option-flag list.
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<Vec<ChatOptionFlag>> for ChatOptionFlags {
    /// Wraps an owned vector without copying.
    fn from(options: Vec<ChatOptionFlag>) -> Self {
        Self(options)
    }
}

impl<'a> IntoIterator for &'a ChatOptionFlags {
    type Item = &'a ChatOptionFlag;
    type IntoIter = std::slice::Iter<'a, ChatOptionFlag>;

    /// Iterates immutably over parser option flags.
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'a> IntoIterator for &'a mut ChatOptionFlags {
    type Item = &'a mut ChatOptionFlag;
    type IntoIter = std::slice::IterMut<'a, ChatOptionFlag>;

    /// Iterates mutably over parser option flags.
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut()
    }
}

impl IntoIterator for ChatOptionFlags {
    type Item = ChatOptionFlag;
    type IntoIter = std::vec::IntoIter<ChatOptionFlag>;

    /// Consumes the wrapper and yields owned option flags.
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl Default for ChatOptionFlags {
    /// Returns an empty option-flag list (no `@Options` header).
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl crate::validation::Validate for ChatOptionFlags {
    /// Enforces non-empty `@Options` (option values are otherwise free-form flags).
    fn validate(
        &self,
        _context: &crate::validation::ValidationContext,
        errors: &impl crate::ErrorSink,
    ) {
        if self.is_empty() {
            errors.report(crate::ParseError::new(
                crate::ErrorCode::EmptyOptionsHeader,
                crate::Severity::Error,
                crate::SourceLocation::at_offset(0),
                crate::ErrorContext::new("", 0..0, "options"),
                "@Options header cannot be empty",
            ));
        }
    }
}

/// Typed payload for a single CHAT header line.
///
/// Validation across *multiple* headers (required presence, ordering, cross-
/// header consistency) lives in `validation/header/**`; this enum models just
/// the parsed payload of one line.
///
/// Reference:
/// - [File Headers](https://talkbank.org/0info/manuals/CHAT.html#File_Headers)
/// - [Begin Header](https://talkbank.org/0info/manuals/CHAT.html#Begin_Header)
/// - [End Header](https://talkbank.org/0info/manuals/CHAT.html#End_Header)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Header {
    // =========================================================================
    // Required Structure Headers
    // =========================================================================
    /// @UTF8 - File encoding declaration (required first header)
    Utf8,

    /// @Begin - Transcript start marker (required)
    Begin,

    /// @End - Transcript end marker (required last)
    End,

    // =========================================================================
    // Participant Headers
    // =========================================================================
    /// @Languages:\tlang1, lang2, ...
    Languages {
        /// Ordered language codes (first is primary)
        codes: LanguageCodes,
    },

    /// @Participants:\tSPK Name Role, SPK2 Name2 Role2, ...
    Participants {
        /// Participant entries (speaker code, optional name, role)
        entries: ParticipantEntries,
    },

    /// @ID:\tlang|corpus|speaker|age|sex|group|ses|role|education|custom|
    #[serde(rename = "id")]
    ID(super::super::IDHeader),

    // =========================================================================
    // Metadata Headers
    // =========================================================================
    /// @Date:\tDD-MMM-YYYY
    Date {
        /// Calendar date in DD-MMM-YYYY format
        date: ChatDate,
    },

    /// @Comment:\tfree text
    Comment {
        /// Free-text comment content, may include a media bullet
        content: crate::model::BulletContent,
    },

    /// @PID:\tpersistent-id
    Pid {
        /// Persistent identifier value
        pid: PidValue,
    },

    /// @Media:\tfilename, type
    #[serde(rename = "media")]
    Media(super::super::MediaHeader),

    /// @Situation:\tdescription
    Situation {
        /// Free-text situation description
        text: SituationDescription,
    },

    /// @Types:\tdesign, activity, group
    #[serde(rename = "types")]
    Types(TypesHeader),

    // =========================================================================
    // Gem Headers (Time-aligned Segments)
    // =========================================================================
    /// @Bg:\tlabel - Begin gem
    BeginGem {
        /// Optional gem label
        label: Option<GemLabel>,
    },

    /// @Eg:\tlabel - End gem
    EndGem {
        /// Optional gem label
        label: Option<GemLabel>,
    },

    /// @G:\tlabel - Lazy gem
    LazyGem {
        /// Optional gem label
        label: Option<GemLabel>,
    },

    // =========================================================================
    // CLAN Display Headers (Pre-@Begin)
    // =========================================================================
    /// @Font:\tspecification
    Font {
        /// Font specification string
        font: FontSpec,
    },

    /// @Window:\tgeometry
    Window {
        /// Window geometry description
        geometry: WindowGeometry,
    },

    /// @Color words:\tcolors
    ColorWords {
        /// Color word palette
        colors: ColorWordList,
    },

    // =========================================================================
    // Recording/Session Headers
    // =========================================================================
    /// @Number:\t1-5|more|audience
    Number {
        /// Participant count
        number: Number,
    },

    /// @Recording Quality:\t1-5
    RecordingQuality {
        /// Quality rating (1-5)
        quality: RecordingQuality,
    },

    /// @Transcription:\ttype
    Transcription {
        /// Transcription quality type
        transcription: Transcription,
    },

    /// @New Episode
    NewEpisode,

    /// @Tape Location:\ttime
    TapeLocation {
        /// Tape location description
        location: TapeLocationDescription,
    },

    /// @Time Duration:\trange
    TimeDuration {
        /// Time duration value
        duration: TimeDurationValue,
    },

    /// @Time Start:\ttime
    TimeStart {
        /// Start time value
        start: TimeStartValue,
    },

    /// @Location:\tplace
    Location {
        /// Recording location description
        location: LocationDescription,
    },

    /// @Room Layout:\tdescription
    RoomLayout {
        /// Room layout description
        layout: RoomLayoutDescription,
    },

    // =========================================================================
    // Participant-Specific Headers
    // =========================================================================
    /// @Birth of SPK:\tDD-MMM-YYYY
    Birth {
        /// Speaker code of the participant
        participant: SpeakerCode,
        /// Birth date in DD-MMM-YYYY format
        date: ChatDate,
    },

    /// @Birthplace of SPK:\tplace
    Birthplace {
        /// Speaker code of the participant
        participant: SpeakerCode,
        /// Birthplace description
        place: BirthplaceDescription,
    },

    /// @L1 of SPK:\tlanguage
    L1Of {
        /// Speaker code of the participant
        participant: SpeakerCode,
        /// First language name
        language: LanguageName,
    },

    // =========================================================================
    // Other Headers
    // =========================================================================
    /// @Blank - Blank line marker
    Blank,

    /// @Transcriber:\tname
    Transcriber {
        /// Transcriber name
        transcriber: TranscriberName,
    },

    /// @Warning:\twarning text
    Warning {
        /// Warning message text
        text: WarningText,
    },

    /// Parse-recovery fallback for unrecognized or malformed header lines.
    ///
    /// This variant should be eliminated by validation (reported as an error)
    /// before data is treated as a clean CHAT model.
    Unknown {
        /// Header text as captured from parser recovery.
        text: WarningText,
        /// Optional parser-side reason describing why decoding failed.
        parse_reason: Option<String>,
        /// Optional suggested correction (for example nearest known header).
        suggested_fix: Option<String>,
    },

    /// @Activities:\tlist
    Activities {
        /// Activities description list
        activities: ActivitiesDescription,
    },

    /// @Bck:\tbackground
    Bck {
        /// Background context description
        bck: BackgroundDescription,
    },

    /// @Options:\toptions (CA, CA-Unicode, bullets)
    Options {
        /// Option flags controlling parser behavior
        options: ChatOptionFlags,
    },

    /// @Page:\tnumber
    Page {
        /// Page number identifier
        page: PageNumber,
    },

    /// @Videos:\tfiles
    Videos {
        /// Video file specification
        videos: VideoSpec,
    },

    /// @T:\ttext (inline thumbnail marker)
    T {
        /// Thumbnail marker text
        text: TDescription,
    },
}
