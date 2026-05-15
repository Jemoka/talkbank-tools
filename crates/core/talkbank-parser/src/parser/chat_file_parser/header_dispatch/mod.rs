//! Header dispatch for single-line `parse_header` API calls.
//!
//! This path parses headers outside full-file context by wrapping input in a
//! minimal synthetic document, then locating and decoding the target header node.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Languages_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#ID_Header>

mod finder;
mod parse;
#[cfg(test)]
mod tests;
