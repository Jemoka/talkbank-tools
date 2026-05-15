//! Typed code wrappers used by CHAT header payloads.
//!
//! These types provide domain-specific wrappers around frequently reused tokens
//! (`SpeakerCode`, `LanguageCode`, participant roles/names, and related header
//! string fields) so parser/validator code does not pass raw strings around.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Languages_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#ID_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Date_Header>

// Submodules
mod age;
mod date;
mod header_strings;
pub(crate) mod iso639;
mod language;
mod participant;
mod ses;
mod speaker;
pub(crate) mod time_values;

// Re-export all public types
pub use age::AgeValue;
pub use date::{ChatDate, Month};
pub use header_strings::*;
pub use language::LanguageCode;
pub use participant::{ParticipantEntry, ParticipantName, ParticipantRole};
pub use ses::{Ethnicity, SesCode, SesValue};
pub use speaker::SpeakerCode;
pub use time_values::{TimeDurationValue, TimeSegment, TimeStartValue, TimeValue};
