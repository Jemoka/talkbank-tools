//! Error code definitions and temporal validation constants.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>

mod error_code;
/// Temporal/media bullet validation constants.
pub mod temporal;

pub use error_code::ErrorCode;
pub use temporal::*;
