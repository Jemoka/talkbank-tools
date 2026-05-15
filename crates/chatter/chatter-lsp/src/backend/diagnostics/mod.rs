//! LSP diagnostic generation and publishing.
//!
//! Converts parse errors and validation errors to LSP Diagnostic messages with
//! context-aware related information, caching, and incremental validation support.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

mod cache_builder;
mod conversion;
mod related_info;
mod text_diff;
mod validation_orchestrator;

// Re-export public API
pub(crate) use validation_orchestrator::{ValidationResources, validate_and_publish};
