//! CHAT file template generation.
//!
//! Provides helpers for generating valid CHAT files with the required prelude.
//!
//! ## Template Types
//!
//! - **`MinimalChatFile`** - Simple single-speaker files (good for basic tests and CLI usage)
//! - **`ChatFileBuilder`** - Full-featured builder for complex validation tests:
//!   - Multiple speakers
//!   - Multiple utterances (for cross-utterance linkers: `[>]`, `[<]`, `[<1]`)
//!   - Bullet timing (for monotonicity validation)
//!   - Dependent tiers (for alignment validation)
//!   - Custom headers
//!
//! ## Examples
//!
//! ### Simple file (CLI or basic tests)
//! ```
//! use talkbank_parser_tests::minimal_chat_file;
//! let content = minimal_chat_file();
//! ```
//!
//! ### Multi-utterance file (linker validation)
//! ```
//! use talkbank_parser_tests::ChatFileBuilder;
//!
//! let content = ChatFileBuilder::new()
//!     .speaker("CHI", "Target_Child")
//!     .utterance("CHI", "this is the first sentence .")
//!     .utterance("CHI", "and [>] this continues it .")
//!     .build();
//! ```
//!
//! ### File with timing (monotonicity validation)
//! ```
//! use talkbank_parser_tests::ChatFileBuilder;
//!
//! let content = ChatFileBuilder::new()
//!     .speaker("CHI", "Target_Child")
//!     .utterance_with_timing("CHI", "hello .", 1000, 2000)
//!     .utterance_with_timing("CHI", "world .", 2500, 3500)
//!     .build();
//! ```
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

mod builder;
mod minimal;

pub use builder::ChatFileBuilder;
pub use minimal::{MinimalChatFile, minimal_chat_file};

#[cfg(test)]
mod tests;
