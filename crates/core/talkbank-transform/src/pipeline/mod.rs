//! Module declarations and re-exports for this subsystem.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

mod convert;
mod error;
mod io;
mod parse;

pub use convert::{chat_to_json, chat_to_json_unvalidated, normalize_chat};
pub use error::PipelineError;
pub use io::parse_file_and_validate;
pub use parse::{
    parse_and_validate, parse_and_validate_streaming, parse_and_validate_streaming_with_parser,
    parse_and_validate_with_parser,
};
