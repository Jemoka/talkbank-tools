//! Core participant model assembled from participant-related headers.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#ID_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Birth_Header>

use super::super::{
    ChatDate, IDHeader, ParticipantEntry, ParticipantName, ParticipantRole, SpeakerCode,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// Participant record assembled from participant-related headers.
///
/// Combines:
/// - `@Participants` entry (`code`, optional `name`, `role`)
/// - `@ID` payload (language/age/sex/resources/corpus/etc.)
/// - optional `@Birth of <CODE>` date
///
/// # CHAT Format Reference
///
/// - [@Participants](https://talkbank.org/0info/manuals/CHAT.html#Participants_Header)
/// - [@ID](https://talkbank.org/0info/manuals/CHAT.html#ID_Header)
/// - [@Birth](https://talkbank.org/0info/manuals/CHAT.html#Birth_Header)
///
/// This type is the source of truth used by validation and JSON export for
/// participant metadata.
///
/// # Example
///
/// ```rust
/// # use talkbank_model::model::{Participant, ParticipantEntry, IDHeader, SpeakerCode, ParticipantRole, ParticipantName};
/// let entry = ParticipantEntry {
///     speaker_code: SpeakerCode::new("CHI"),
///     name: Some(ParticipantName::new("Ruth")),
///     role: ParticipantRole::new("Target_Child"),
/// };
///
/// let id = IDHeader::new("eng", "CHI", "Target_Child");
///
/// let participant = Participant::new(entry, id);
/// assert_eq!(participant.code.as_str(), "CHI");
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct Participant {
    /// Speaker code used on main tiers (`*CHI: ...`).
    ///
    /// This code is used in main tier lines to identify the speaker:
    /// ```chat
    /// *CHI:    hello .
    /// ```
    pub code: SpeakerCode,

    /// Optional human-readable name from `@Participants`.
    pub name: Option<ParticipantName>,

    /// Role token from `@Participants`.
    pub role: ParticipantRole,

    /// Full `@ID` metadata payload.
    pub id: IDHeader,

    /// Optional date from `@Birth of <CODE>`.
    pub birth_date: Option<ChatDate>,
}

impl Participant {
    /// Construct a participant from `@Participants` entry plus `@ID`.
    ///
    /// Primary constructor used by parsers. Birth date, if present, is attached
    /// later via [`Self::with_birth_date`].
    ///
    /// # Arguments
    ///
    /// - `entry`: parsed `@Participants` entry
    /// - `id`: parsed `@ID` data for the same speaker code
    ///
    /// # Example
    ///
    /// ```rust
    /// # use talkbank_model::model::{Participant, ParticipantEntry, IDHeader, SpeakerCode, ParticipantRole, ParticipantName};
    /// let entry = ParticipantEntry {
    ///     speaker_code: SpeakerCode::new("CHI"),
    ///     name: Some(ParticipantName::new("Ruth")),
    ///     role: ParticipantRole::new("Target_Child"),
    /// };
    ///
    /// let id = IDHeader::new("eng", "CHI", "Target_Child");
    ///
    /// let participant = Participant::new(entry, id);
    /// assert_eq!(participant.code.as_str(), "CHI");
    /// assert_eq!(
    ///     participant.name.as_ref().map(|n| n.as_str()),
    ///     Some("Ruth")
    /// );
    /// assert_eq!(participant.role.as_str(), "Target_Child");
    /// assert_eq!(participant.birth_date, None);
    /// ```
    pub fn new(entry: ParticipantEntry, id: IDHeader) -> Self {
        Self {
            code: entry.speaker_code,
            name: entry.name,
            role: entry.role,
            id,
            birth_date: None,
        }
    }

    /// Attach optional `@Birth of <CODE>` date.
    ///
    /// # Arguments
    ///
    /// - `date`: parsed CHAT date value
    ///
    /// # Example
    ///
    /// ```rust
    /// # use talkbank_model::model::{Participant, ParticipantEntry, IDHeader, SpeakerCode, ParticipantRole, ChatDate};
    /// # let entry = ParticipantEntry {
    /// #     speaker_code: SpeakerCode::new("CHI"),
    /// #     name: None,
    /// #     role: ParticipantRole::new("Target_Child"),
    /// # };
    /// # let id = IDHeader::new("eng", "CHI", "Target_Child");
    /// let participant = Participant::new(entry, id)
    ///     .with_birth_date(ChatDate::new("28-JUN-2001"));
    ///
    /// assert_eq!(
    ///     participant.birth_date.as_ref().map(|d| d.as_str()),
    ///     Some("28-JUN-2001")
    /// );
    /// ```
    pub fn with_birth_date(mut self, date: ChatDate) -> Self {
        self.birth_date = Some(date);
        self
    }
}
