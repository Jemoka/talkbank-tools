//! Shared validation-cache setup and access helpers.

use std::path::Path;
use std::sync::Arc;

use talkbank_transform::UnifiedCache;

use crate::commands::CacheRefreshMode;

/// Shared cache handle used by validation entrypoints.
pub(crate) type ValidationCacheHandle = Arc<UnifiedCache>;

/// Create the validation cache and apply `--force` clearing for the target path when requested.
pub(crate) fn initialize_validation_cache(
    path: &Path,
    cache_refresh: CacheRefreshMode,
) -> Option<ValidationCacheHandle> {
    match UnifiedCache::new() {
        Ok(cache) => {
            let cache = Arc::new(cache);

            if cache_refresh.should_clear_cache() {
                let path_str = path.to_string_lossy();
                match cache.clear_prefix(&path_str) {
                    Ok(count) => {
                        eprintln!("Cleared {} cache entries", count);
                    }
                    Err(error) => {
                        eprintln!("Warning: Failed to clear cache: {}", error);
                    }
                }
            }

            Some(cache)
        }
        Err(error) => {
            eprintln!("Warning: Failed to initialize cache: {}", error);
            None
        }
    }
}

/// Return one cached validation result when available.
pub(crate) fn get_cached_validation(
    cache: Option<&ValidationCacheHandle>,
    path: &Path,
    check_alignment: bool,
) -> Option<bool> {
    cache.and_then(|cache| cache.get_validation(path, check_alignment))
}

/// Store one validation result, warning on cache-write failures.
pub(crate) fn set_cached_validation(
    cache: Option<&ValidationCacheHandle>,
    path: &Path,
    check_alignment: bool,
    valid: bool,
) {
    if let Some(cache) = cache
        && let Err(error) = cache.set_validation(path, check_alignment, valid)
    {
        eprintln!("Warning: Failed to cache validation results: {}", error);
    }
}
