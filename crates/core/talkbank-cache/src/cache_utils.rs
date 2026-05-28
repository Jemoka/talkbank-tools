//! Utility functions for cache operations.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use super::error::CacheError;

/// Get unique cache key for a file path with a suffix.
pub fn get_cache_key_with_suffix(path: &Path, suffix: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    path.to_string_lossy().hash(&mut hasher);
    suffix.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

/// Compute blake3 content hash of a file, returned as a hex string.
pub fn get_content_hash(path: &Path) -> Result<String, CacheError> {
    let data = std::fs::read(path).map_err(|source| CacheError::Metadata {
        path: path.display().to_string(),
        source,
    })?;
    Ok(blake3::hash(&data).to_hex().to_string())
}

/// Get current time in seconds since epoch.
pub fn now_secs() -> Result<u64, CacheError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|e| CacheError::Message(format!("system time before Unix epoch: {e}")))
}

/// Get the default cache directory.
pub fn default_cache_dir() -> Result<std::path::PathBuf, CacheError> {
    dirs::cache_dir()
        .map(|d| d.join("talkbank-chat"))
        .ok_or(CacheError::CacheDirMissing)
}
