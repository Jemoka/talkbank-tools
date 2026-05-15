//! Structural parsing for one CHAT main tier line.
//!
//! This layer decodes speaker prefix, content/body, utterance terminator, and
//! end-of-utterance adjuncts (postcodes and trailing bullets) before handing off
//! word-level details to content/word parsers.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Terminators>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Postcodes>

pub mod contents;
mod convert;
pub mod errors;
pub mod finder;
pub(crate) mod terminator;
mod utterance_end;

pub use convert::convert_main_tier_node;
pub use errors::collect_main_tier_errors;
pub use finder::find_main_tier_node;
pub use utterance_end::parse_utterance_end;
