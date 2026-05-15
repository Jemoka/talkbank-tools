//! Regression tests for validation-runner orchestration behavior.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use super::{
    CacheMode, CacheOutcome, ValidationCache, ValidationConfig, ValidationEvent,
    validate_directory_streaming,
};
use std::fs;
use std::path::Path;
use std::time::Duration;
use tempfile::tempdir;

struct NoopCache;

impl ValidationCache for NoopCache {
    fn get(&self, _path: &Path, _check_alignment: bool) -> Option<CacheOutcome> {
        None
    }

    fn set(
        &self,
        _path: &Path,
        _check_alignment: bool,
        _outcome: CacheOutcome,
    ) -> Result<(), String> {
        Ok(())
    }
}

/// Cache that records what was stored, for verifying cache semantics.
struct RecordingCache {
    stored: std::sync::Mutex<Vec<(std::path::PathBuf, CacheOutcome)>>,
}

impl RecordingCache {
    fn new() -> Self {
        Self {
            stored: std::sync::Mutex::new(Vec::new()),
        }
    }

    fn outcomes(&self) -> Vec<(std::path::PathBuf, CacheOutcome)> {
        self.stored.lock().unwrap().clone()
    }
}

impl ValidationCache for RecordingCache {
    fn get(&self, _path: &Path, _check_alignment: bool) -> Option<CacheOutcome> {
        None
    }

    fn set(
        &self,
        path: &Path,
        _check_alignment: bool,
        outcome: CacheOutcome,
    ) -> Result<(), String> {
        self.stored
            .lock()
            .unwrap()
            .push((path.to_path_buf(), outcome));
        Ok(())
    }
}

/// Regression test: a file producing only warnings (no errors) must be cached
/// as Invalid so warnings are shown on every run. Previously, warnings-only
/// files were cached as Valid, silently hiding warnings on subsequent runs.
#[test]
fn warnings_only_file_cached_as_invalid() {
    let dir = tempdir().expect("create temp dir");
    // This file triggers E546 warning (unsupported SES value "badses") but is
    // otherwise valid CHAT — producing warnings but no errors.
    let file_path = dir.path().join("warnings.cha");
    fs::write(
        &file_path,
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|3;06.|female||badses|Target_Child|||\n*CHI:\thello world .\n@End\n",
    )
    .expect("write test chat file with warning-producing SES value");

    let cache = std::sync::Arc::new(RecordingCache::new());
    let config = ValidationConfig {
        jobs: Some(1),
        cache: CacheMode::Enabled,
        ..ValidationConfig::default()
    };

    let (events, _cancel_tx) =
        validate_directory_streaming(dir.path(), &config, Some(cache.clone()));

    // Drain events to completion.
    let mut error_events = Vec::new();
    loop {
        let event = events
            .recv_timeout(Duration::from_secs(10))
            .expect("runner should finish");
        match event {
            ValidationEvent::Errors(e) => error_events.push(e),
            ValidationEvent::Finished(_) => break,
            _ => {}
        }
    }

    // The file should produce at least one warning (E546).
    assert!(
        !error_events.is_empty(),
        "file with unsupported SES should produce warning events"
    );

    // The file must be cached as Invalid so warnings are shown on subsequent runs.
    let outcomes = cache.outcomes();
    assert_eq!(outcomes.len(), 1, "exactly one file should be cached");
    assert_eq!(
        outcomes[0].1,
        CacheOutcome::Invalid,
        "warnings-only file must be cached as Invalid to prevent hiding warnings"
    );
}

/// A valid file with no warnings should be cached as Valid.
#[test]
fn clean_file_cached_as_valid() {
    let dir = tempdir().expect("create temp dir");
    let file_path = dir.path().join("clean.cha");
    fs::write(
        &file_path,
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|demo|CHI|2;00.00|male|||Target_Child|||\n*CHI:\thello .\n@End\n",
    )
    .expect("write clean test chat file");

    let cache = std::sync::Arc::new(RecordingCache::new());
    let config = ValidationConfig {
        jobs: Some(1),
        cache: CacheMode::Enabled,
        ..ValidationConfig::default()
    };

    let (events, _cancel_tx) =
        validate_directory_streaming(dir.path(), &config, Some(cache.clone()));

    loop {
        let event = events
            .recv_timeout(Duration::from_secs(10))
            .expect("runner should finish");
        if matches!(event, ValidationEvent::Finished(_)) {
            break;
        }
    }

    let outcomes = cache.outcomes();
    assert_eq!(outcomes.len(), 1, "exactly one file should be cached");
    assert_eq!(
        outcomes[0].1,
        CacheOutcome::Valid,
        "clean file should be cached as Valid"
    );
}

#[test]
fn dropped_cancel_sender_does_not_cancel_and_jobs_zero_still_processes_files() {
    let dir = tempdir().expect("create temp dir");
    let file_path = dir.path().join("sample.cha");
    fs::write(
        &file_path,
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|demo|CHI|2;00.00|male|||Target_Child|||\n*CHI:\thello .\n@End\n",
    )
    .expect("write test chat file");

    let config = ValidationConfig {
        jobs: Some(0),
        cache: CacheMode::Disabled,
        roundtrip: false,
        ..ValidationConfig::default()
    };

    let (events, cancel_tx) = validate_directory_streaming::<NoopCache>(dir.path(), &config, None);
    drop(cancel_tx);

    let mut file_complete_count = 0usize;
    let finished = loop {
        let event = events
            .recv_timeout(Duration::from_secs(10))
            .expect("runner should emit events and finish");
        match event {
            ValidationEvent::FileComplete(_) => {
                file_complete_count += 1;
            }
            ValidationEvent::Finished(stats) => break stats,
            _ => {}
        }
    };

    assert_eq!(
        file_complete_count, 1,
        "exactly one file should be processed"
    );
    assert!(
        !finished.cancelled,
        "dropping cancel sender must not cancel run"
    );
    assert_eq!(
        finished.valid_files + finished.invalid_files + finished.parse_errors,
        1,
        "one file should be accounted for in final stats"
    );
}
