//! Complete CHAT transcript representation.
//!
//! `ChatFile` preserves the original interleaving of header lines and utterances.
//! This gives deterministic roundtrip behavior and keeps positional context for
//! diagnostics that depend on file order.
//!
//! # CHAT Format Structure
//!
//! Headers and utterances are interleaved in real corpora:
//!
//! ```text
//! @UTF8
//! @Begin
//! @Languages: eng
//! @Participants: CHI Target_Child, MOT Mother
//! *CHI: hello .
//! @Comment: This comment appears between utterances
//! *MOT: hi there !
//! @Comment: Another interleaved comment
//! @End
//! ```
//!
//! # CHAT Format Reference
//!
//! - [CHAT Manual](https://talkbank.org/0info/manuals/CHAT.html)
//! - [File Headers](https://talkbank.org/0info/manuals/CHAT.html#File_Headers)
//! - [Main Lines](https://talkbank.org/0info/manuals/CHAT.html#Main_Line)

mod accessors;
mod core;
mod validate;
mod write;

pub use core::{ChatFile, ChatFileLines};
