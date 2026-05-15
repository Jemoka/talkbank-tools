//! Integration tests for the CachePool (in-memory SQLite cache).
//!
//! The cache reads file content hashes from disk, so all tests use real
//! temp files created by `tempfile`.

use std::io::Write as _;
use std::path::Path;
use talkbank_transform::unified_cache::CacheError;
use talkbank_transform::{CacheOutcome, CachePool, ValidationCache};

/// Create a temp file with the given content, returning its path.
/// The `dir` must outlive the test to keep the file on disk.
fn write_temp_cha(dir: &Path, name: &str, content: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    let mut f = std::fs::File::create(&path).expect("create temp file");
    f.write_all(content.as_bytes())
        .expect("write temp file content");
    path
}

const CHAT_CONTENT: &str = "@UTF8\n@Begin\n@End\n";
const CHAT_CONTENT_B: &str = "@UTF8\n@Begin\n@Languages:\teng\n@End\n";

// ===== Basic operations (6 tests) =====

#[test]
fn cache_set_and_get_valid() -> Result<(), CacheError> {
    let dir = tempfile::tempdir().map_err(|e| CacheError::Message(e.to_string()))?;
    let path = write_temp_cha(dir.path(), "valid.cha", CHAT_CONTENT);

    let cache = CachePool::in_memory()?;
    cache.set_validation(&path, false, true)?;
    let result = cache.get_validation(&path, false);
    assert_eq!(result, Some(true), "Should retrieve cached valid result");
    Ok(())
}

#[test]
fn cache_set_and_get_invalid() -> Result<(), CacheError> {
    let dir = tempfile::tempdir().map_err(|e| CacheError::Message(e.to_string()))?;
    let path = write_temp_cha(dir.path(), "invalid.cha", CHAT_CONTENT);

    let cache = CachePool::in_memory()?;
    cache.set_validation(&path, false, false)?;
    let result = cache.get_validation(&path, false);
    assert_eq!(result, Some(false), "Should retrieve cached invalid result");
    Ok(())
}

#[test]
fn cache_miss_returns_none() -> Result<(), CacheError> {
    let dir = tempfile::tempdir().map_err(|e| CacheError::Message(e.to_string()))?;
    let path = write_temp_cha(dir.path(), "uncached.cha", CHAT_CONTENT);

    let cache = CachePool::in_memory()?;
    // Do NOT set anything — query should return None
    let result = cache.get_validation(&path, false);
    assert_eq!(result, None, "Uncached path should return None");
    Ok(())
}

#[test]
fn cache_alignment_flag_is_key() -> Result<(), CacheError> {
    let dir = tempfile::tempdir().map_err(|e| CacheError::Message(e.to_string()))?;
    let path = write_temp_cha(dir.path(), "align.cha", CHAT_CONTENT);

    let cache = CachePool::in_memory()?;
    // Store valid for alignment=false, invalid for alignment=true
    cache.set_validation(&path, false, true)?;
    cache.set_validation(&path, true, false)?;

    assert_eq!(
        cache.get_validation(&path, false),
        Some(true),
        "alignment=false should return valid"
    );
    assert_eq!(
        cache.get_validation(&path, true),
        Some(false),
        "alignment=true should return invalid"
    );
    Ok(())
}

#[test]
fn cache_roundtrip_parser_kind_is_key() -> Result<(), CacheError> {
    let dir = tempfile::tempdir().map_err(|e| CacheError::Message(e.to_string()))?;
    let path = write_temp_cha(dir.path(), "parser.cha", CHAT_CONTENT);

    let cache = CachePool::in_memory()?;
    cache.set_roundtrip(&path, false, "tree-sitter", true)?;
    cache.set_roundtrip(&path, false, "re2c", false)?;

    assert_eq!(
        cache.get_roundtrip(&path, false, "tree-sitter"),
        Some(true),
        "tree-sitter roundtrip should be valid"
    );
    assert_eq!(
        cache.get_roundtrip(&path, false, "re2c"),
        Some(false),
        "re2c roundtrip should be invalid"
    );
    Ok(())
}

#[test]
fn cache_clear_all() -> Result<(), CacheError> {
    let dir = tempfile::tempdir().map_err(|e| CacheError::Message(e.to_string()))?;
    let path = write_temp_cha(dir.path(), "clearme.cha", CHAT_CONTENT);

    let cache = CachePool::in_memory()?;
    cache.set_validation(&path, false, true)?;
    assert_eq!(cache.get_validation(&path, false), Some(true));

    cache.clear_all()?;
    assert_eq!(
        cache.get_validation(&path, false),
        None,
        "After clear_all, get should return None"
    );
    Ok(())
}

// ===== Maintenance (3 tests) =====

#[test]
fn cache_clear_prefix() -> Result<(), CacheError> {
    let dir = tempfile::tempdir().map_err(|e| CacheError::Message(e.to_string()))?;

    // Create files under two different sub-directories
    let corpus_a = dir.path().join("corpus_a");
    let corpus_b = dir.path().join("corpus_b");
    std::fs::create_dir_all(&corpus_a).map_err(|e| CacheError::Message(e.to_string()))?;
    std::fs::create_dir_all(&corpus_b).map_err(|e| CacheError::Message(e.to_string()))?;

    let path_a1 = write_temp_cha(&corpus_a, "file1.cha", CHAT_CONTENT);
    let path_a2 = write_temp_cha(&corpus_a, "file2.cha", CHAT_CONTENT);
    let path_b1 = write_temp_cha(&corpus_b, "file3.cha", CHAT_CONTENT);

    let cache = CachePool::in_memory()?;
    cache.set_validation(&path_a1, false, true)?;
    cache.set_validation(&path_a2, false, true)?;
    cache.set_validation(&path_b1, false, false)?;

    let cleared = cache.clear_prefix(corpus_a.to_str().ok_or(CacheError::CacheDirMissing)?)?;
    assert_eq!(cleared, 2, "Should clear 2 entries matching prefix");

    assert_eq!(
        cache.get_validation(&path_a1, false),
        None,
        "Cleared entry should be gone"
    );
    assert_eq!(
        cache.get_validation(&path_b1, false),
        Some(false),
        "Non-matching prefix should remain"
    );
    Ok(())
}

#[test]
fn cache_stats_count() -> Result<(), CacheError> {
    let dir = tempfile::tempdir().map_err(|e| CacheError::Message(e.to_string()))?;
    let p1 = write_temp_cha(dir.path(), "s1.cha", CHAT_CONTENT);
    let p2 = write_temp_cha(dir.path(), "s2.cha", CHAT_CONTENT_B);
    let p3 = write_temp_cha(dir.path(), "s3.cha", "@UTF8\n@Begin\n@End\n");

    let cache = CachePool::in_memory()?;
    cache.set_validation(&p1, false, true)?;
    cache.set_validation(&p2, false, false)?;
    cache.set_validation(&p3, true, true)?;

    let stats = cache.stats()?;
    assert_eq!(stats.total_entries, 3, "Stats should reflect 3 entries");
    Ok(())
}

#[test]
fn cache_overwrite_entry() -> Result<(), CacheError> {
    let dir = tempfile::tempdir().map_err(|e| CacheError::Message(e.to_string()))?;
    let path = write_temp_cha(dir.path(), "overwrite.cha", CHAT_CONTENT);

    let cache = CachePool::in_memory()?;
    cache.set_validation(&path, false, true)?;
    assert_eq!(cache.get_validation(&path, false), Some(true));

    // Overwrite with opposite result
    cache.set_validation(&path, false, false)?;
    assert_eq!(
        cache.get_validation(&path, false),
        Some(false),
        "Overwritten entry should return new value"
    );
    Ok(())
}

// ===== Edge cases (3 tests) =====

#[test]
fn cache_in_memory_creation() {
    let result = CachePool::in_memory();
    assert!(result.is_ok(), "in_memory() should not fail");
}

#[test]
fn cache_stats_after_clear() -> Result<(), CacheError> {
    let dir = tempfile::tempdir().map_err(|e| CacheError::Message(e.to_string()))?;
    let path = write_temp_cha(dir.path(), "x.cha", CHAT_CONTENT);

    let cache = CachePool::in_memory()?;
    cache.set_validation(&path, false, true)?;
    cache.clear_all()?;

    let stats = cache.stats()?;
    assert_eq!(
        stats.total_entries, 0,
        "Stats should be zero after clear_all"
    );
    Ok(())
}

#[test]
fn cache_multiple_paths_independent() -> Result<(), CacheError> {
    let dir = tempfile::tempdir().map_err(|e| CacheError::Message(e.to_string()))?;
    let path_a = write_temp_cha(dir.path(), "a.cha", CHAT_CONTENT);
    let path_b = write_temp_cha(dir.path(), "b.cha", CHAT_CONTENT_B);

    let cache = CachePool::in_memory()?;
    cache.set_validation(&path_a, false, true)?;
    cache.set_validation(&path_b, false, false)?;

    assert_eq!(cache.get_validation(&path_a, false), Some(true));
    assert_eq!(cache.get_validation(&path_b, false), Some(false));

    // Verify the ValidationCache trait also works
    let outcome_a = ValidationCache::get(&cache, &path_a, false);
    assert_eq!(outcome_a, Some(CacheOutcome::Valid));

    let outcome_b = ValidationCache::get(&cache, &path_b, false);
    assert_eq!(outcome_b, Some(CacheOutcome::Invalid));
    Ok(())
}
