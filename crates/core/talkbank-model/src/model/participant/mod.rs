//! Participant model combining `@Participants`, `@ID`, and optional `@Birth`.
//!
//! In CHAT, participant metadata is split across multiple header families:
//! - `@Participants`: code, optional name, role
//! - `@ID`: language/resources/corpus/age/sex plus optional study metadata
//! - `@Birth of <CODE>`: optional birth date
//!
//! This module exposes a unified [`Participant`] view so downstream code does
//! not need to re-join those headers manually.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#ID_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Birth_Header>
//!
//! # CHAT Requirement
//!
//! Every participant listed in `@Participants` must have a corresponding `@ID`.
//! Parsing/validation enforces this invariant.
//!
//! # Example CHAT Structure
//!
//! ```chat
//! @Participants:    CHI Ruth Target_Child, INV Chiat Investigator
//! @ID:    eng|chiat|CHI|10;03.||||Target_Child|||
//! @ID:    eng|chiat|INV|||||Investigator|||
//! @Birth of CHI:    28-JUN-2001
//! ```
//!
//! This produces two `Participant` values with merged metadata.
//!
//! # Example Usage
//!
//! ```rust
//! use talkbank_model::model::{
//!     Participant,
//!     ParticipantEntry,
//!     IDHeader,
//!     SpeakerCode,
//!     ParticipantRole,
//!     ParticipantName,
//!     ChatDate,
//! };
//!
//! let entry = ParticipantEntry {
//!     speaker_code: SpeakerCode::new("CHI"),
//!     name: Some(ParticipantName::new("Ruth")),
//!     role: ParticipantRole::new("Target_Child"),
//! };
//!
//! let id = IDHeader::new("eng", "CHI", "Target_Child")
//!     .with_age("10;03.")
//!     .with_corpus("chiat");
//!
//! let participant = Participant::new(entry, id)
//!     .with_birth_date(ChatDate::new("28-JUN-2001"));
//!
//! assert_eq!(participant.code.as_str(), "CHI");
//! assert_eq!(
//!     participant.name.as_ref().map(|n| n.as_str()),
//!     Some("Ruth")
//! );
//! assert_eq!(
//!     participant.birth_date.as_ref().map(|d| d.as_str()),
//!     Some("28-JUN-2001")
//! );
//! ```

mod accessors;
mod core;
#[cfg(test)]
mod tests;

pub use core::Participant;
