//! Metadata header parsing (`@Languages`, `@PID`, `@Media`, `@Situation`, `@Types`).
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Languages_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#PID_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Media_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Situation_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Types_Header>

mod languages;
mod media;
mod pid;
mod situation;
mod t_header;
mod types;

pub use languages::parse_languages_header;
pub use media::parse_media_header;
pub use pid::parse_pid_header;
pub use situation::parse_situation_header;
pub use t_header::parse_t_header;
pub use types::parse_types_header;
