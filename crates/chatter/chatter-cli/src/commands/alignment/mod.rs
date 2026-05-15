//! Alignment visualization command.
//!
//! This module boots the `show_alignment` helper that renders `%mor/%gra/%pho` alignment
//! tables for debugging. It keeps the CLI-facing command simple while deferring payload loading
//! and rendering to `show::load` and `show::render`.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

mod helpers;
mod show;

pub use show::show_alignment;
