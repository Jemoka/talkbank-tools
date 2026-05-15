//! Corpus operations for CHAT file collections.
//!
//! This module provides functionality for:
//! - **Discovery**: Finding and scanning corpus directories
//! - **Manifest**: Tracking file status and test results
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

pub mod discovery;
pub mod manifest;

// Re-export public types for convenience
pub use discovery::{build_manifest, corpus_summary, discover_corpora, format_manifest};
pub use manifest::{
    CorpusEntry, CorpusManifest, FailureReason, FileEntry, FileStatus, ManifestError,
};
