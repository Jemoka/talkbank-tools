//! Shared validation runner for both CLI and GUI
//!
//! This module provides a streaming validation system using channels.
//! Events stream as they happen, enabling real-time progress and error display.
//! Supports cancellation via a cancel channel.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//!
//! ## Architecture
//!
//! Errors are batched at the file level:
//! - All errors for a file are collected during validation
//! - One `Errors` event is sent per file (if file has errors)
//! - Then `FileComplete` event is sent
//!
//! This ensures clean output (one header per file) and bounded memory.

mod cache;
mod config;
mod helpers;
pub mod roundtrip;
mod runner;
#[cfg(test)]
mod tests;
mod types;
mod worker;

// Re-export public API
pub use cache::{CacheOutcome, ValidationCache};
pub use config::{CacheMode, DirectoryMode, ParserKind, ValidationConfig};
pub use runner::{validate_directory_streaming, validate_files_streaming};
pub use types::{
    ErrorEvent, FileCompleteEvent, FileStatus, RoundtripEvent, ValidationEvent, ValidationStats,
    ValidationStatsSnapshot,
};
