//! Unified SQLite cache for validation and roundtrip test results (pass/fail only).
//!
//! Lives in its own crate (separate from `talkbank-transform`) so that products
//! that don't need a validation cache (notably `batchalign`, which uses a
//! `redb`-backed cache in `batchalign-engine`) don't transitively pull `sqlx`
//! and `libsqlite3-sys` into their wheel build.
//!
//! Implements the [`talkbank_transform::ValidationCache`] trait; consumers
//! call into it through that abstraction.
//!
//! The cache answers one question: "Has this file already been validated/roundtrip-tested
//! at this mtime and tool version?" — returning just pass/fail.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

mod error;
mod types;

// Utility and infrastructure modules
mod cache_utils;
mod schema_init;

// Operation modules
mod maintenance_ops;
mod roundtrip_ops;
mod validation_ops;

// Core implementation
mod cache_impl;

// Re-export public API
pub use cache_impl::CachePool;
pub use error::CacheError;
pub use types::CacheStats;

/// Backward-compatible alias. Prefer `CachePool` in new code.
pub type UnifiedCache = CachePool;
