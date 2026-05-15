//! Replacement annotation parsing
//!
//! Handles [: word1 word2 ...] and [:: word1 word2 ...] constructs
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Retracing_and_Repetition>

mod helpers;
mod parse;

pub(crate) use parse::parse_replacement;
