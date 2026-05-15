//! Participant map building from headers
//!
//! Matches @Participants entries with @ID headers and optional @Birth headers.
//!
//! # CHAT Format Requirements
//!
//! According to the CHAT manual, every participant listed in @Participants MUST have
//! a corresponding @ID header. This module enforces that requirement during parsing.
//!
//! # Example Headers
//!
//! ```chat
//! @Participants:    CHI Ruth Target_Child, INV Chiat Investigator
//! @ID:    eng|chiat|CHI|10;03.||||Target_Child|||
//! @ID:    eng|chiat|INV|||||Investigator|||
//! @Birth of CHI:    28-JUN-2001
//! ```
//!
//! This creates two participants:
//! - CHI: with name "Ruth", role "Target_Child", age "10;03.", birth date "28-JUN-2001"
//! - INV: with name "Chiat", role "Investigator"
//!
//! # Error Handling
//!
//! - E522: Missing @ID for participant (Error)
//! - E523: Orphan @ID without @Participants entry (Warning)
//! - E524: @Birth for unknown participant (Warning)
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#ID_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Birth_Header>

mod birth;
mod builder;

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) use builder::build_participants;
pub use builder::build_participants_from_lines;
