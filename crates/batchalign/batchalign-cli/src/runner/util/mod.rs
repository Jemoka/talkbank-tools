//! Utility functions, constants, and auto-tuning for the runner module.

mod auto_tune;
pub(crate) mod batch_progress;
mod error_classification;
mod file_status;
mod media;

// Re-export everything at the same paths callers already use.
pub use auto_tune::KNOWN_MEDIA_EXTENSIONS;
pub(super) use auto_tune::compute_job_workers;

pub(crate) use error_classification::classify_server_error;
pub(super) use error_classification::user_facing_error;
pub(crate) use error_classification::{classify_worker_error, is_retryable_worker_failure};

pub(crate) use file_status::{
    FileRunTracker, FileStage, FileTaskOutcome, ProgressSender, ProgressUpdate, RunnerEventSink,
    StoreRunnerEventSink, set_file_progress,
};
pub(super) use file_status::{
    drain_supervised_file_tasks, force_terminal_file_states, spawn_progress_forwarder,
    spawn_supervised_file_task,
};

#[cfg(test)]
pub(super) use media::apply_result_filename;
#[cfg(test)]
pub(super) use media::resolve_audio_for_chat;
pub(super) use media::{
    collect_preflight_audio_paths, compute_audio_identity, get_audio_duration_ms,
    preflight_validate_media, resolve_audio_for_chat_with_media_dir, should_preflight,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::{AsrEngineName, FaEngineName};
    use std::collections::BTreeMap;
    use std::path::Path;

    use crate::api::{DisplayPath, JobId, LanguageCode3, NumSpeakers, NumWorkers, ReleasedCommand};
    use crate::config::ServerConfig;
    use crate::host_facts::EffectiveConfig;
    use crate::options::{AlignOptions, CommandOptions, CommonOptions, UtrEngine};
    use crate::runtime;

    /// Resolve an [`EffectiveConfig`] for a test against the live
    /// host's detected facts — same wiring as production
    /// `DispatchHostContext::from_store`. Tests on Apple Silicon vs
    /// CUDA hosts will see different `gpu_thread_pool_size`
    /// recommendations, so the `compute_workers_*` assertions use
    /// inequality bounds (`<=`) where the cap matters.
    fn effective_for_test(config: &ServerConfig) -> EffectiveConfig {
        EffectiveConfig::resolve_from_server_config(config)
    }
    use crate::scheduling::FailureCategory;
    use crate::store::{
        PendingJobFile, RunnerDispatchConfig, RunnerFilesystemConfig, RunnerJobIdentity,
        RunnerJobSnapshot,
    };
    use crate::worker::error::WorkerError;
    use tokio_util::sync::CancellationToken;

    #[test]
    fn apply_result_filename_basic() {
        use std::path::PathBuf;
        assert_eq!(
            apply_result_filename(std::path::Path::new("/out/dir/audio.wav"), "audio.cha"),
            PathBuf::from("/out/dir/audio.cha")
        );
    }

    #[test]
    fn apply_result_filename_preserves_dir() {
        use std::path::PathBuf;
        assert_eq!(
            apply_result_filename(
                std::path::Path::new("/data/resources/corpus/output/test.mp3"),
                "test.cha"
            ),
            PathBuf::from("/data/resources/corpus/output/test.cha")
        );
    }

    #[test]
    fn compute_workers_single_file() {
        let config = ServerConfig::default();
        let effective = effective_for_test(&config);
        assert_eq!(
            compute_job_workers(ReleasedCommand::Morphotag, 1, &effective, &config),
            NumWorkers(1)
        );
    }

    #[test]
    fn compute_workers_explicit_config() {
        // The explicit cap is a CEILING. `compute_job_workers` also
        // clamps by `available_parallelism()` and per-category caps,
        // both of which are host-derived and shrink on small CI
        // runners. The right invariant is `1 <= result <= explicit`
        // — same shape as `compute_workers_auto_tune_caps_at_max`
        // below. Asserting `==` overspecified and only worked on
        // many-core dev machines.
        let config = ServerConfig {
            max_workers_per_job: Some(3),
            ..Default::default()
        };
        let effective = effective_for_test(&config);
        let result = compute_job_workers(ReleasedCommand::Morphotag, 10, &effective, &config);
        assert!(*result >= 1 && *result <= 3, "got {result:?}");
    }

    #[test]
    fn compute_workers_explicit_caps_at_file_count() {
        // Same reasoning as `compute_workers_explicit_config`:
        // ceiling, not equality. The intent is "explicit cap of 8 is
        // honored AND further clamped to file count of 2".
        let config = ServerConfig {
            max_workers_per_job: Some(8),
            ..Default::default()
        };
        let effective = effective_for_test(&config);
        let result = compute_job_workers(ReleasedCommand::Morphotag, 2, &effective, &config);
        assert!(*result >= 1 && *result <= 2, "got {result:?}");
    }

    #[test]
    fn compute_workers_auto_tune_caps_at_max() {
        let config = ServerConfig::default();
        let effective = effective_for_test(&config);
        // Auto-tune should never exceed runtime::max_thread_workers()
        let result = compute_job_workers(ReleasedCommand::Opensmile, 100, &effective, &config);
        assert!(*result <= runtime::max_thread_workers());
        assert!(*result >= 1);
    }

    #[test]
    fn compute_workers_gpu_commands_use_thread_pool_cap() {
        let config = ServerConfig {
            gpu_thread_pool_size: Some(2),
            ..Default::default()
        };
        let effective = effective_for_test(&config);
        let result = compute_job_workers(ReleasedCommand::Transcribe, 47, &effective, &config);
        assert!(
            *result <= 2,
            "GPU worker count must respect gpu_thread_pool_size, got {result}"
        );
    }

    #[test]
    fn compute_workers_gpu_commands_respect_medium_tier_cap() {
        let config = ServerConfig {
            memory_tier: Some(crate::types::runtime::MemoryTierKind::Medium),
            gpu_thread_pool_size: Some(4),
            ..Default::default()
        };
        let effective = effective_for_test(&config);
        let result = compute_job_workers(ReleasedCommand::Align, 7, &effective, &config);
        assert_eq!(
            result,
            NumWorkers(1),
            "Medium tier GPU jobs should not exceed the tier worker cap (1 for LazyProfile)"
        );
    }

    #[test]
    fn compute_workers_cpu_commands_respect_medium_tier_cap() {
        let config = ServerConfig {
            memory_tier: Some(crate::types::runtime::MemoryTierKind::Medium),
            ..Default::default()
        };
        let effective = effective_for_test(&config);
        let result = compute_job_workers(ReleasedCommand::Opensmile, 100, &effective, &config);
        assert_eq!(
            result,
            NumWorkers(1),
            "Medium tier auto-tune should not exceed the tier worker cap (1 for LazyProfile)"
        );
    }

    #[test]
    fn command_execution_budgets_follow_runtime_constants() {
        use batchalign_types::api::ReleasedCommand;
        use batchalign_types::command_spec::command_spec_for;

        // In test environment PYTHON_GIL is not set to "0", so the process
        // (conservative) budget table is used by default (not free-threaded).
        let free_threaded = false;

        let morphotag = command_spec_for(ReleasedCommand::Morphotag);
        assert_eq!(
            (morphotag.base_mb_for_runtime(free_threaded).0 as f64 * morphotag.loading_overhead.0)
                as u64,
            (8_000f64 * 1.5) as u64,
        );

        let align = command_spec_for(ReleasedCommand::Align);
        assert_eq!(
            (align.base_mb_for_runtime(free_threaded).0 as f64 * align.loading_overhead.0) as u64,
            (4_000f64 * 1.5) as u64,
        );

        let opensmile = command_spec_for(ReleasedCommand::Opensmile);
        assert_eq!(
            (opensmile.base_mb_for_runtime(free_threaded).0 as f64 * opensmile.loading_overhead.0)
                as u64,
            (500f64 * 1.5) as u64,
        );
    }

    #[test]
    fn worker_error_classification_is_stable() {
        assert_eq!(
            classify_worker_error(&WorkerError::ProcessExited {
                code: Some(9),
                stderr: None
            }),
            FailureCategory::WorkerCrash
        );
        assert_eq!(
            classify_worker_error(&WorkerError::ReadyTimeout { timeout_s: 30 }),
            FailureCategory::WorkerTimeout
        );
        assert_eq!(
            classify_worker_error(&WorkerError::Protocol(
                "timeout waiting for infer response".into()
            )),
            FailureCategory::WorkerTimeout
        );
        assert_eq!(
            classify_worker_error(&WorkerError::Protocol("bad frame".into())),
            FailureCategory::WorkerProtocol
        );
        assert_eq!(
            classify_worker_error(&WorkerError::WorkerResponse("temporary".into())),
            FailureCategory::ProviderTransient
        );
        assert!(is_retryable_worker_failure(FailureCategory::WorkerCrash));
        assert!(is_retryable_worker_failure(FailureCategory::WorkerTimeout));
        assert!(is_retryable_worker_failure(
            FailureCategory::ProviderTransient
        ));
        assert!(!is_retryable_worker_failure(
            FailureCategory::WorkerProtocol
        ));
        assert!(!is_retryable_worker_failure(FailureCategory::Validation));
    }

    /// Regression: bootstrap-class errors must NOT be retried.
    ///
    /// Historical context: a Stanza-catalog-missing failure during worker
    /// bootstrap was classified as a transient `WorkerCrash` and retried
    /// 3× with a full Python traceback per attempt. On a long-running
    /// daemon this produced multi-GB log explosions from a deterministic
    /// config error. The fix: the Python worker surfaces typed bootstrap
    /// failures via the `{"op":"error", "kind":"bootstrap"}` wire shape;
    /// the Rust side classifies these as terminal so the orchestrator
    /// stops retrying.
    #[test]
    fn bootstrap_errors_are_non_retryable() {
        let category = classify_worker_error(&WorkerError::Bootstrap(
            "Failed to download Stanza resource catalog from \
             https://raw.githubusercontent.com/stanfordnlp/stanza-resources/main: \
             connection refused"
                .into(),
        ));
        assert!(
            !is_retryable_worker_failure(category),
            "Bootstrap-class errors must be terminal at the worker layer; \
             retrying a deterministic config error 3× is the bug shape this \
             test is pinning. Got category: {category:?}"
        );
    }

    #[test]
    fn process_exited_display_includes_stderr_when_present() {
        let error = WorkerError::ProcessExited {
            code: Some(1),
            stderr: Some("Traceback (most recent call last):\n  File \"worker.py\", line 42\ntorch.cuda.OutOfMemoryError: CUDA out of memory".into()),
        };
        let msg = error.to_string();
        assert!(
            msg.contains("exit code: Some(1)"),
            "should show exit code: {msg}"
        );
        assert!(
            msg.contains("OutOfMemoryError"),
            "should include stderr tail: {msg}"
        );
        assert!(
            msg.contains("--- worker stderr ---"),
            "should have stderr header: {msg}"
        );
    }

    #[test]
    fn process_exited_display_without_stderr_is_clean() {
        let error = WorkerError::ProcessExited {
            code: Some(9),
            stderr: None,
        };
        let msg = error.to_string();
        assert!(
            msg.contains("exit code: Some(9)"),
            "should show exit code: {msg}"
        );
        assert!(!msg.contains("stderr"), "should not mention stderr: {msg}");
    }

    #[test]
    fn worker_crash_user_facing_error_includes_raw_detail() {
        let raw = "FA processing failed: worker process exited unexpectedly (exit code: Some(-9))\n--- worker stderr ---\nKilled: 9";
        let msg = user_facing_error(FailureCategory::WorkerCrash, "Alignment", "test.cha", raw);
        assert!(
            msg.contains("Killed: 9"),
            "should include stderr detail: {msg}"
        );
        assert!(msg.contains("test.cha"), "should include filename: {msg}");
    }

    #[test]
    fn should_preflight_transcribe_default() {
        assert!(should_preflight(ReleasedCommand::Transcribe, None));
    }

    #[test]
    fn should_preflight_transcribe_rev_explicit() {
        use crate::options::{CommonOptions, TranscribeOptions};
        let opts = CommandOptions::Transcribe(TranscribeOptions {
            common: CommonOptions::default(),
            asr_engine: AsrEngineName::RevAi,
            diarize: false,
            wor: false.into(),
            merge_abbrev: false.into(),
            batch_size: 4,
            utseg_fallback: false.into(),
        });
        assert!(should_preflight(ReleasedCommand::Transcribe, Some(&opts)));
    }

    #[test]
    fn should_preflight_transcribe_whisper() {
        use crate::options::{CommonOptions, TranscribeOptions};
        let opts = CommandOptions::Transcribe(TranscribeOptions {
            common: CommonOptions::default(),
            asr_engine: AsrEngineName::Whisper,
            diarize: false,
            wor: false.into(),
            merge_abbrev: false.into(),
            batch_size: 4,
            utseg_fallback: false.into(),
        });
        assert!(!should_preflight(ReleasedCommand::Transcribe, Some(&opts)));
    }

    #[test]
    fn should_preflight_benchmark() {
        assert!(should_preflight(ReleasedCommand::Benchmark, None));
    }

    #[test]
    fn should_preflight_align_default() {
        assert!(should_preflight(ReleasedCommand::Align, None));
    }

    #[test]
    fn should_preflight_align_no_utr() {
        use crate::options::{AlignOptions, CommonOptions};
        let opts = CommandOptions::Align(AlignOptions {
            common: CommonOptions::default(),
            fa_engine: FaEngineName::Wave2Vec,
            utr_engine: None,
            utr_overlap_strategy: Default::default(),
            utr_two_pass: Default::default(),
            pauses: false,
            wor: true.into(),
            merge_abbrev: false.into(),
            media_dir: None,
            bullet_repair: false,
            review_level: Default::default(),
        });
        assert!(!should_preflight(ReleasedCommand::Align, Some(&opts)));
    }

    #[test]
    fn should_preflight_morphotag() {
        assert!(!should_preflight(ReleasedCommand::Morphotag, None));
    }

    #[tokio::test]
    async fn media_validate_not_paths_mode() {
        let file_list = vec![PendingJobFile {
            file_index: 0,
            filename: "audio.wav".into(),
            has_chat: false,
        }];
        let source_paths = vec![batchalign_types::paths::ClientPath::new(
            "/nonexistent/audio.wav",
        )];
        let failures = preflight_validate_media(&file_list, &source_paths, false).await;
        assert!(
            failures.is_empty(),
            "Should skip validation when not paths_mode"
        );
    }

    #[tokio::test]
    async fn media_validate_skips_chat_files() {
        let file_list = vec![PendingJobFile {
            file_index: 0,
            filename: "test.cha".into(),
            has_chat: true,
        }];
        let source_paths = vec![batchalign_types::paths::ClientPath::new(
            "/nonexistent/test.cha",
        )];
        let failures = preflight_validate_media(&file_list, &source_paths, true).await;
        assert!(failures.is_empty(), "Should skip CHAT files");
    }

    #[tokio::test]
    async fn media_validate_missing_file() {
        let file_list = vec![PendingJobFile {
            file_index: 0,
            filename: "missing.wav".into(),
            has_chat: false,
        }];
        let source_paths = vec![batchalign_types::paths::ClientPath::new(
            "/nonexistent/path/missing.wav",
        )];
        let failures = preflight_validate_media(&file_list, &source_paths, true).await;
        assert_eq!(failures.len(), 1);
        assert!(failures[&0].contains("not found"));
    }

    #[tokio::test]
    async fn media_validate_empty_file() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        // File is empty (0 bytes)
        let path = tmp.path().to_string_lossy().to_string();
        // Rename to .wav for extension check
        let wav_path = format!("{path}.wav");
        std::fs::rename(&path, &wav_path).unwrap();
        let file_list = vec![PendingJobFile {
            file_index: 0,
            filename: "test.wav".into(),
            has_chat: false,
        }];
        let source_paths = vec![batchalign_types::paths::ClientPath::new(&wav_path)];
        let failures = preflight_validate_media(&file_list, &source_paths, true).await;
        assert_eq!(failures.len(), 1);
        assert!(failures[&0].contains("empty"));
        let _ = std::fs::remove_file(&wav_path);
    }

    #[tokio::test]
    async fn media_validate_unknown_extension() {
        let mut tmp = tempfile::NamedTempFile::with_suffix(".xyz").unwrap();
        std::io::Write::write_all(&mut tmp, b"data").unwrap();
        let path = tmp.path().to_string_lossy().to_string();
        let file_list = vec![PendingJobFile {
            file_index: 0,
            filename: "test.xyz".into(),
            has_chat: false,
        }];
        let source_paths = vec![batchalign_types::paths::ClientPath::new(&path)];
        let failures = preflight_validate_media(&file_list, &source_paths, true).await;
        assert_eq!(failures.len(), 1);
        assert!(failures[&0].contains("Unknown media format"));
    }

    #[tokio::test]
    async fn media_validate_valid_file() {
        let mut tmp = tempfile::NamedTempFile::with_suffix(".wav").unwrap();
        std::io::Write::write_all(&mut tmp, b"RIFF....").unwrap();
        let path = tmp.path().to_string_lossy().to_string();
        let file_list = vec![PendingJobFile {
            file_index: 0,
            filename: "test.wav".into(),
            has_chat: false,
        }];
        let source_paths = vec![batchalign_types::paths::ClientPath::new(&path)];
        let failures = preflight_validate_media(&file_list, &source_paths, true).await;
        assert!(failures.is_empty(), "Valid .wav file should pass");
    }

    #[tokio::test]
    async fn media_validate_all_known_extensions() {
        for ext in KNOWN_MEDIA_EXTENSIONS {
            let mut tmp = tempfile::NamedTempFile::with_suffix(format!(".{ext}")).unwrap();
            std::io::Write::write_all(&mut tmp, b"data").unwrap();
            let path = tmp.path().to_string_lossy().to_string();
            let file_list = vec![PendingJobFile {
                file_index: 0,
                filename: DisplayPath::from(format!("test.{ext}")),
                has_chat: false,
            }];
            let source_paths = vec![batchalign_types::paths::ClientPath::new(&path)];
            let failures = preflight_validate_media(&file_list, &source_paths, true).await;
            assert!(failures.is_empty(), "Extension .{ext} should be accepted");
        }
    }

    // -----------------------------------------------------------------------
    // Audio resolution for FA (content mode)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn resolve_audio_finds_wav_alongside_cha() {
        let dir = tempfile::tempdir().unwrap();
        let cha = dir.path().join("test.cha");
        let wav = dir.path().join("test.wav");
        std::fs::write(&cha, "@Begin\n@End\n").unwrap();
        std::fs::write(&wav, b"RIFF").unwrap();

        let result = resolve_audio_for_chat(&cha).await;
        assert!(result.is_some(), "Should find wav alongside cha");
        assert!(result.unwrap().ends_with("test.wav"));
    }

    #[tokio::test]
    async fn resolve_audio_returns_none_when_no_media() {
        let dir = tempfile::tempdir().unwrap();
        let cha = dir.path().join("test.cha");
        std::fs::write(&cha, "@Begin\n@End\n").unwrap();

        let result = resolve_audio_for_chat(&cha).await;
        assert!(
            result.is_none(),
            "Should return None when no audio file exists"
        );
    }

    #[tokio::test]
    async fn collect_preflight_audio_paths_resolves_align_media_from_chat_inputs() {
        let dir = tempfile::tempdir().unwrap();
        let chat_path = dir.path().join("sample.cha");
        let wav_path = dir.path().join("sample.wav");
        std::fs::write(&chat_path, "@Begin\n@End\n").unwrap();
        std::fs::write(&wav_path, b"RIFF").unwrap();

        let job = RunnerJobSnapshot {
            identity: RunnerJobIdentity {
                job_id: JobId::from("job-1"),
                correlation_id: "corr-1".into(),
            },
            dispatch: RunnerDispatchConfig {
                command: ReleasedCommand::Align,
                lang: crate::api::LanguageSpec::Resolved(LanguageCode3::eng()),
                num_speakers: NumSpeakers(1),
                options: CommandOptions::Align(AlignOptions {
                    common: CommonOptions::default(),
                    fa_engine: FaEngineName::Wave2Vec,
                    utr_engine: Some(UtrEngine::RevAi),
                    utr_overlap_strategy: Default::default(),
                    utr_two_pass: Default::default(),
                    pauses: false,
                    wor: true.into(),
                    merge_abbrev: false.into(),
                    media_dir: None,
                    bullet_repair: false,
                    review_level: Default::default(),
                }),
                runtime_state: BTreeMap::new(),
                debug_traces: false,
            },
            filesystem: RunnerFilesystemConfig {
                paths_mode: true,
                source_paths: vec![batchalign_types::paths::ClientPath::new(
                    chat_path.to_string_lossy().to_string(),
                )],
                output_paths: vec![],
                before_paths: vec![],
                staging_dir: Default::default(),
                media_mapping: Default::default(),
                media_subdir: Default::default(),
                source_dir: Default::default(),
            },
            cancel_token: CancellationToken::new(),
            pending_files: vec![PendingJobFile {
                file_index: 0,
                filename: DisplayPath::from("sample.cha"),
                has_chat: true,
            }],
        };

        let paths =
            collect_preflight_audio_paths(ReleasedCommand::Align, &job, &job.pending_files).await;

        assert_eq!(paths, vec![wav_path.to_path_buf()]);
    }

    /// Simulates the content-mode FA audio resolution bug: staged file in a
    /// temp dir has no audio alongside it, but the client's source_dir does.
    /// Verifies that looking at source_dir finds the audio.
    #[tokio::test]
    async fn resolve_audio_source_dir_fallback() {
        // Simulate client's source directory with audio
        let source_dir = tempfile::tempdir().unwrap();
        let _cha_source = source_dir.path().join("ACWT01a.cha");
        let wav_source = source_dir.path().join("ACWT01a.wav");
        std::fs::write(&_cha_source, "@Begin\n@End\n").unwrap();
        std::fs::write(&wav_source, b"RIFF").unwrap();

        // Simulate server staging directory (no audio here)
        let staging_dir = tempfile::tempdir().unwrap();
        let staged_cha = staging_dir.path().join("ACWT01a.cha");
        std::fs::write(&staged_cha, "@Begin\n@End\n").unwrap();

        // Strategy 4 (alongside staged file): should fail
        let staged_result = resolve_audio_for_chat(&staged_cha).await;
        assert!(staged_result.is_none(), "No audio in staging dir");

        // Strategy 2 (source_dir): should succeed — this is the fix
        let source_path = source_dir.path().join("ACWT01a.cha");
        let source_result = resolve_audio_for_chat(&source_path).await;
        assert!(source_result.is_some(), "Should find audio via source_dir");
        assert!(source_result.unwrap().ends_with("ACWT01a.wav"));
    }

    /// Verifies media_roots fallback: audio not alongside cha or in source_dir,
    /// but present in a configured media_root.
    #[tokio::test]
    async fn resolve_audio_media_roots_fallback() {
        let media_root = tempfile::tempdir().unwrap();
        let wav = media_root.path().join("ACWT01a.wav");
        std::fs::write(&wav, b"RIFF").unwrap();

        let stem = "ACWT01a";
        let mut found = None;
        let roots = vec![media_root.path().to_string_lossy().to_string()];
        'roots: for root in &roots {
            for ext in KNOWN_MEDIA_EXTENSIONS {
                let candidate = Path::new(root).join(format!("{stem}.{ext}"));
                if tokio::fs::try_exists(&candidate).await.unwrap_or(false) {
                    found = Some(candidate.to_string_lossy().to_string());
                    break 'roots;
                }
            }
        }
        assert!(found.is_some(), "Should find audio in media_root");
        assert!(found.unwrap().ends_with("ACWT01a.wav"));
    }
}
