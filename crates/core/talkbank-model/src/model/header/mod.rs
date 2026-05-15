//! Header-layer model for CHAT `@...` lines.
//!
//! This module groups typed header representations and supporting value types
//! used across parsing, validation, and serialization.
//!
//! Layout:
//! - `header_enum`: canonical `Header` enum for all recognized header lines
//! - `codes`: typed code/newtype wrappers used inside headers
//! - `id`: structured `@ID` payload type
//! - `media`: structured `@Media` payload type
//! - `types_header`: `@Types` payload types
//! - `enums`: shared enum values used by header payloads
//! - `write_chat`: CHAT serialization for header values
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Languages_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#ID_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Media_Header>

mod codes;
mod enums;
mod header_enum;
mod id;
mod media;
mod types_header;
mod write_chat;

// Re-export everything from the split modules
pub use codes::*;
pub use enums::*;
pub use header_enum::{ChatOptionFlag, ChatOptionFlags, Header, LanguageCodes, ParticipantEntries};
pub use id::IDHeader;
pub use media::MediaHeader;
pub use types_header::*;

// Re-export for submodules via super::
pub(crate) use crate::model::WriteChat;

// Crate-internal re-export for shared time parsing used by dependent_tier::tim.
pub(crate) use codes::time_values::parse_time_value;
