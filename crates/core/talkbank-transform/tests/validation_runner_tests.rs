//! Integration tests for the streaming validation runner, config, stats, and corpus discovery.

use std::io::Write as _;
use talkbank_transform::{
    CacheMode, DirectoryMode, ParserKind, ValidationConfig, ValidationEvent, ValidationStats,
    build_manifest, corpus_summary, validate_directory_streaming,
};

/// Minimal valid CHAT file content for temp directory tests.
const VALID_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\thello world .\n%mor:\tn|hello n|world .\n@End\n";

/// Invalid CHAT content (missing @End).
const INVALID_CHAT: &str =
    "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n*CHI:\thello .\n";

// ===== Config (3 tests) =====

#[test]
fn default_config_values() {
    let config = ValidationConfig::default();
    assert!(
        config.check_alignment,
        "Default should enable alignment checking"
    );
    assert!(!config.roundtrip, "Default should disable roundtrip");
    assert_eq!(config.cache, CacheMode::Enabled);
    assert_eq!(config.directory, DirectoryMode::Recursive);
}

#[test]
fn config_parser_kind_default_is_treesitter() {
    let config = ValidationConfig::default();
    assert_eq!(
        config.parser_kind,
        ParserKind::TreeSitter,
        "Default parser kind should be TreeSitter"
    );
}

#[test]
fn config_cache_mode_default_is_enabled() {
    let config = ValidationConfig::default();
    assert_eq!(
        config.cache,
        CacheMode::Enabled,
        "Default cache mode should be Enabled"
    );
}

// ===== Stats (4 tests) =====

#[test]
fn stats_initial_zero() {
    let stats = ValidationStats::new(10);
    let snap = stats.snapshot();
    assert_eq!(snap.total_files, 10);
    assert_eq!(snap.valid_files, 0);
    assert_eq!(snap.invalid_files, 0);
    assert_eq!(snap.cache_hits, 0);
    assert_eq!(snap.cache_misses, 0);
    assert_eq!(snap.parse_errors, 0);
    assert!(!snap.cancelled);
}

#[test]
fn stats_record_valid_increments() {
    let stats = ValidationStats::new(5);
    stats.record_valid_file();
    stats.record_valid_file();
    let snap = stats.snapshot();
    assert_eq!(snap.valid_files, 2);
}

#[test]
fn stats_record_invalid_increments() {
    let stats = ValidationStats::new(5);
    stats.record_invalid_file();
    stats.record_invalid_file();
    stats.record_invalid_file();
    let snap = stats.snapshot();
    assert_eq!(snap.invalid_files, 3);
}

#[test]
fn stats_snapshot_is_consistent() {
    let stats = ValidationStats::new(10);
    stats.record_valid_file();
    stats.record_valid_file();
    stats.record_invalid_file();
    stats.record_cache_hit();
    stats.record_cache_miss();
    stats.record_parse_error();
    stats.record_roundtrip_passed();
    stats.record_roundtrip_failed();

    let snap = stats.snapshot();
    assert_eq!(snap.total_files, 10);
    assert_eq!(snap.valid_files, 2);
    assert_eq!(snap.invalid_files, 1);
    assert_eq!(snap.cache_hits, 1);
    assert_eq!(snap.cache_misses, 1);
    assert_eq!(snap.parse_errors, 1);
    assert_eq!(snap.roundtrip_passed, 1);
    assert_eq!(snap.roundtrip_failed, 1);
    assert!(!snap.cancelled);

    // Cache hit rate: 1 hit out of 10 total = 10%
    assert!((snap.cache_hit_rate() - 10.0).abs() < 0.01);
}

// ===== Streaming validation (3 tests) =====

/// Helper: write a .cha file into a directory.
fn write_cha_file(dir: &std::path::Path, name: &str, content: &str) {
    let path = dir.join(name);
    let mut f = std::fs::File::create(&path).ok();
    if let Some(ref mut f) = f {
        f.write_all(content.as_bytes()).ok();
    }
}

#[test]
fn validate_directory_with_valid_files() {
    let dir = tempfile::tempdir().ok();
    let dir = match dir {
        Some(ref d) => d.path(),
        None => return,
    };

    write_cha_file(dir, "valid1.cha", VALID_CHAT);
    write_cha_file(dir, "valid2.cha", VALID_CHAT);

    let config = ValidationConfig {
        check_alignment: false,
        jobs: Some(1),
        cache: CacheMode::Disabled,
        directory: DirectoryMode::Recursive,
        roundtrip: false,
        parser_kind: ParserKind::TreeSitter,
        strict_linkers: false,
    };

    let (events, _cancel) =
        validate_directory_streaming::<talkbank_transform::CachePool>(dir, &config, None);

    let mut saw_started = false;
    let mut saw_finished = false;
    for event in events {
        match event {
            ValidationEvent::Started { total_files } => {
                assert_eq!(total_files, 2);
                saw_started = true;
            }
            ValidationEvent::Finished(snap) => {
                saw_finished = true;
                assert_eq!(snap.total_files, 2);
            }
            _ => {}
        }
    }
    assert!(saw_started, "Should see Started event");
    assert!(saw_finished, "Should see Finished event");
}

#[test]
fn validate_directory_with_invalid_file() {
    let dir = tempfile::tempdir().ok();
    let dir = match dir {
        Some(ref d) => d.path(),
        None => return,
    };

    write_cha_file(dir, "bad.cha", INVALID_CHAT);

    let config = ValidationConfig {
        check_alignment: false,
        jobs: Some(1),
        cache: CacheMode::Disabled,
        directory: DirectoryMode::Recursive,
        roundtrip: false,
        parser_kind: ParserKind::TreeSitter,
        strict_linkers: false,
    };

    let (events, _cancel) =
        validate_directory_streaming::<talkbank_transform::CachePool>(dir, &config, None);

    let mut saw_errors = false;
    let mut saw_finished = false;
    for event in events {
        match event {
            ValidationEvent::Errors(_) => {
                saw_errors = true;
            }
            ValidationEvent::Finished(snap) => {
                saw_finished = true;
                // The file should be counted as invalid or have parse errors
                assert!(
                    snap.invalid_files > 0 || snap.parse_errors > 0,
                    "Invalid file should be counted: invalid={}, parse_errors={}",
                    snap.invalid_files,
                    snap.parse_errors
                );
            }
            _ => {}
        }
    }
    assert!(saw_finished, "Should see Finished event");
    // Errors event should fire for the invalid file
    assert!(saw_errors, "Should see Errors event for invalid CHAT file");
}

#[test]
fn validate_directory_empty() {
    let dir = tempfile::tempdir().ok();
    let dir = match dir {
        Some(ref d) => d.path(),
        None => return,
    };

    let config = ValidationConfig {
        check_alignment: false,
        jobs: Some(1),
        cache: CacheMode::Disabled,
        directory: DirectoryMode::Recursive,
        roundtrip: false,
        parser_kind: ParserKind::TreeSitter,
        strict_linkers: false,
    };

    let (events, _cancel) =
        validate_directory_streaming::<talkbank_transform::CachePool>(dir, &config, None);

    let mut saw_started = false;
    let mut saw_finished = false;
    for event in events {
        match event {
            ValidationEvent::Started { total_files } => {
                assert_eq!(total_files, 0);
                saw_started = true;
            }
            ValidationEvent::Finished(snap) => {
                assert_eq!(snap.total_files, 0);
                saw_finished = true;
            }
            _ => {}
        }
    }
    assert!(saw_started, "Empty dir should still emit Started{{0}}");
    assert!(saw_finished, "Empty dir should still emit Finished");
}

// ===== Corpus (3 tests) =====

#[test]
fn build_manifest_from_directory() {
    let dir = tempfile::tempdir().ok();
    let dir = match dir {
        Some(ref d) => d.path(),
        None => return,
    };

    // build_manifest looks for directories with 0metadata.cdc
    let corpus_dir = dir.join("TestCorpus");
    std::fs::create_dir_all(&corpus_dir).ok();
    std::fs::write(corpus_dir.join("0metadata.cdc"), "test metadata").ok();
    write_cha_file(&corpus_dir, "file1.cha", VALID_CHAT);
    write_cha_file(&corpus_dir, "file2.cha", VALID_CHAT);

    let manifest = build_manifest(dir);
    assert!(manifest.is_ok(), "build_manifest should succeed");
    let manifest = manifest.ok();
    if let Some(m) = manifest {
        assert_eq!(m.total_corpora, 1, "Should find one corpus");
        assert_eq!(m.total_files, 2, "Should find two .cha files");
    }
}

#[test]
fn build_manifest_empty_dir() {
    let dir = tempfile::tempdir().ok();
    let dir = match dir {
        Some(ref d) => d.path(),
        None => return,
    };

    let manifest = build_manifest(dir);
    assert!(manifest.is_ok());
    let manifest = manifest.ok();
    if let Some(m) = manifest {
        assert_eq!(m.total_corpora, 0);
        assert_eq!(m.total_files, 0);
    }
}

#[test]
fn corpus_summary_includes_count() {
    let dir = tempfile::tempdir().ok();
    let dir = match dir {
        Some(ref d) => d.path(),
        None => return,
    };

    let corpus_dir = dir.join("SummaryCorpus");
    std::fs::create_dir_all(&corpus_dir).ok();
    std::fs::write(corpus_dir.join("0metadata.cdc"), "metadata").ok();
    write_cha_file(&corpus_dir, "a.cha", VALID_CHAT);

    let manifest = build_manifest(dir);
    if let Ok(m) = manifest {
        let summary = corpus_summary(&m);
        assert!(
            summary.contains("Total files: 1"),
            "Summary should include file count, got: {}",
            summary
        );
        assert!(
            summary.contains("Total corpora: 1"),
            "Summary should include corpus count, got: {}",
            summary
        );
    }
}
