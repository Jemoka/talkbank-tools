//! `Header` enum and related helpers for all recognized CHAT file headers.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Languages_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#ID_Header>
//!
//! This module is the canonical typed boundary between parsed header lines and
//! downstream validation/serialization logic. Keeping all known headers in one
//! enum makes it straightforward to preserve file-order roundtrips while still
//! handling unknown/legacy headers explicitly.

mod header;
mod impls;
mod options;

pub use header::{ChatOptionFlags, Header, LanguageCodes, ParticipantEntries};
pub use options::ChatOptionFlag;
