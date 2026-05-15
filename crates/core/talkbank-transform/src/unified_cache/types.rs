//! Type definitions for unified cache
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>

use std::path::PathBuf;

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Total number of file entries in the cache database.
    pub total_entries: usize,
    /// Filesystem path to the cache directory.
    pub cache_dir: PathBuf,
}
