//! Header parsing for full-file traversal.
//!
//! This layer decodes header CST nodes encountered during `parse_chat_file`.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Languages_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#ID_Header>

mod dispatch;
pub(crate) mod helpers;
mod pre_begin;

pub use dispatch::parse_header_node;
pub use pre_begin::handle_pre_begin_header;
