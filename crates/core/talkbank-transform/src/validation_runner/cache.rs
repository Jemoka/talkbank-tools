//! Validation cache trait
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

use std::path::Path;

/// Outcome of a cached validation or roundtrip check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheOutcome {
    /// The file passed validation (or roundtrip).
    Valid,
    /// The file failed validation (or roundtrip).
    Invalid,
}

/// Validation cache trait - implement this to provide caching
///
/// Uses interior mutability (&self) for concurrent access.
/// Returns pass/fail outcomes only — error details are not cached.
pub trait ValidationCache: Send + Sync {
    /// Get cached validation result, or `None` on cache miss.
    fn get(&self, path: &Path, check_alignment: bool) -> Option<CacheOutcome>;

    /// Store a validation outcome.
    fn set(&self, path: &Path, check_alignment: bool, outcome: CacheOutcome) -> Result<(), String>;

    /// Get cached roundtrip result, or `None` on cache miss.
    ///
    /// Default implementation returns `None` (no caching).
    fn get_roundtrip(
        &self,
        _path: &Path,
        _check_alignment: bool,
        _parser_kind: &str,
    ) -> Option<CacheOutcome> {
        None
    }

    /// Store a roundtrip outcome.
    ///
    /// Default implementation is a no-op.
    fn set_roundtrip(
        &self,
        _path: &Path,
        _check_alignment: bool,
        _parser_kind: &str,
        _outcome: CacheOutcome,
    ) -> Result<(), String> {
        Ok(())
    }
}
