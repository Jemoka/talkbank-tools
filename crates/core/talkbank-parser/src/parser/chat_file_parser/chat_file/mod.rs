//! Entry points for complete CHAT-file parsing.
//!
//! This directory provides both strict (`ParseResult`) and streaming (`ErrorSink`)
//! parse modes over the same CST traversal and recovery logic.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

mod helpers;
pub(crate) mod normalize;
mod parse;
mod streaming;
#[cfg(test)]
mod tests;
