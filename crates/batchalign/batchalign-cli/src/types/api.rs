//! REST API request/response types — mirrors `batchalign/serve/models.py`.
//!
//! Field names use `snake_case` in Rust and serialize to `snake_case` JSON,
//! matching the Pydantic models exactly.
//!
//! This module re-exports types from focused sub-modules so that existing
//! `use crate::api::*` imports continue to work unchanged.

// Re-export everything from the sub-modules so the public API surface is
// identical to what it was before the split.

pub use super::cancellation::*;
pub use super::domain::*;
pub use super::request::*;
pub use super::response::*;
pub use super::status::*;
pub use crate::runner::util::batch_progress::{BatchInferProgress, LanguageGroupProgress};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_status_roundtrip() {
        for status in [
            JobStatus::Queued,
            JobStatus::Running,
            JobStatus::Completed,
            JobStatus::Failed,
            JobStatus::Cancelled,
            JobStatus::Interrupted,
        ] {
            let json = serde_json::to_string(&status).unwrap();
            let back: JobStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, back);
        }
    }

    #[test]
    fn job_status_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&JobStatus::Queued).unwrap(),
            "\"queued\""
        );
        assert_eq!(
            serde_json::to_string(&JobStatus::Running).unwrap(),
            "\"running\""
        );
        assert_eq!(
            serde_json::to_string(&JobStatus::Completed).unwrap(),
            "\"completed\""
        );
    }

    #[test]
    fn job_submission_roundtrip() {
        let json = r#"{"command": "morphotag", "options": {"command": "morphotag"}}"#;
        let sub: JobSubmission = serde_json::from_str(json).unwrap();
        assert_eq!(sub.command, "morphotag");
        // Default value for `lang` on a morphotag deserialization. The
        // wire-level fixture above does not specify `lang`, so we get the
        // serde default: a `LanguageSpec` constructed from the empty
        // string falls back to `Resolved(eng)` via parse_from_db's invalid
        // path. This roundtrip test only exercises the serde shape — it
        // does NOT exercise submission validation, which would (correctly)
        // reject this combination at the route boundary because morphotag
        // requires `LanguageSpec::PerFile`.
        assert_eq!(sub.lang, LanguageSpec::Resolved(LanguageCode3::eng()));
        assert_eq!(sub.num_speakers, 1);
        assert!(sub.files.is_empty());
        assert!(!sub.paths_mode);
        assert_eq!(sub.options.command_name(), "morphotag");
    }

    #[test]
    fn job_control_plane_roundtrip() {
        let info = JobInfo {
            job_id: "job-temporal".into(),
            status: JobStatus::Running,
            command: ReleasedCommand::Morphotag,
            options: crate::options::CommandOptions::Morphotag(crate::options::MorphotagOptions {
                common: crate::options::CommonOptions::default(),

                ..Default::default()
            }),
            lang: LanguageSpec::Resolved(LanguageCode3::eng()),
            source_dir: "/tmp".into(),
            total_files: 1,
            completed_files: 0,
            current_file: None,
            error: None,
            file_statuses: Vec::new(),
            submitted_at: None,
            submitted_by: None,
            submitted_by_name: None,
            completed_at: None,
            duration_s: None,
            next_eligible_at: None,
            num_workers: None,
            active_lease: None,
            batch_progress: None,
            control_plane: Some(JobControlPlaneInfo::temporal_with_execution(
                TemporalWorkflowExecutionInfo {
                    workflow_id: "job-temporal".into(),
                    run_id: Some("run-123".into()),
                    status: Some("running".into()),
                    task_queue: Some("batchalign3-server".into()),
                    history_length: Some(12),
                    describe_error: None,
                },
            )),
            execution_plan: None,
            last_cancelled_at: None,
            last_cancelled_source: None,
            last_cancelled_host: None,
            last_cancelled_reason: None,
        };

        let json = serde_json::to_string(&info).unwrap();
        let back: JobInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(info, back);
    }

    #[test]
    fn job_submission_paths_mode_validation() {
        use crate::options::{CommandOptions, CommonOptions, MorphotagOptions};

        let morphotag_opts = CommandOptions::Morphotag(MorphotagOptions {
            common: CommonOptions::default(),

            ..Default::default()
        });

        let mut sub = JobSubmission {
            command: ReleasedCommand::Morphotag,
            // Morphotag takes no `--lang` and resolves per-file; this is the
            // only legal lang shape for a morphotag submission. The paired
            // `validate_lang_command_pairing()` check rejects anything else.
            lang: LanguageSpec::PerFile,
            num_speakers: NumSpeakers(1),
            files: vec![],
            media_files: vec![],
            media_mapping: Default::default(),
            media_subdir: Default::default(),
            source_dir: Default::default(),
            options: morphotag_opts,
            paths_mode: true,
            source_paths: vec![],
            output_paths: vec![],
            display_names: vec![],
            debug_traces: false,
            before_paths: vec![],
        };

        // paths_mode with empty paths should fail
        assert!(sub.validate().is_err());

        // Mismatched lengths should fail
        sub.source_paths = vec!["/a".into()];
        sub.output_paths = vec!["/b".into(), "/c".into()];
        assert!(sub.validate().is_err());

        // Valid paths_mode
        sub.output_paths = vec!["/b".into()];
        assert!(sub.validate().is_ok());

        // before_paths with mismatched length should fail
        sub.before_paths = vec!["/before_a".into(), "/before_b".into()];
        assert!(sub.validate().is_err());

        // before_paths matching source_paths length should pass
        sub.before_paths = vec!["/before_a".into()];
        assert!(sub.validate().is_ok());

        // empty before_paths always valid (no incremental)
        sub.before_paths = vec![];
        assert!(sub.validate().is_ok());

        // paths_mode with files should fail
        sub.files = vec![FilePayload {
            filename: "test.cha".into(),
            content: "content".into(),
        }];
        assert!(sub.validate().is_err());
    }

    #[test]
    fn file_status_kind_roundtrip() {
        for kind in [
            FileStatusKind::Queued,
            FileStatusKind::Processing,
            FileStatusKind::Done,
            FileStatusKind::Error,
            FileStatusKind::Interrupted,
        ] {
            let json = serde_json::to_string(&kind).unwrap();
            let back: FileStatusKind = serde_json::from_str(&json).unwrap();
            assert_eq!(kind, back);
        }
    }

    #[test]
    fn file_progress_stage_roundtrip() {
        for stage in [
            FileProgressStage::Processing,
            FileProgressStage::Aligning,
            FileProgressStage::BuildingChat,
            FileProgressStage::RetryScheduled,
        ] {
            let json = serde_json::to_string(&stage).unwrap();
            let back: FileProgressStage = serde_json::from_str(&json).unwrap();
            assert_eq!(stage, back);
        }
    }

    #[test]
    fn file_status_kind_display_and_parse() {
        for kind in [
            FileStatusKind::Queued,
            FileStatusKind::Processing,
            FileStatusKind::Done,
            FileStatusKind::Error,
            FileStatusKind::Interrupted,
        ] {
            let s = kind.to_string();
            let parsed: FileStatusKind = s.parse().unwrap();
            assert_eq!(kind, parsed);
        }
    }

    #[test]
    fn job_status_display_and_parse() {
        for status in [
            JobStatus::Queued,
            JobStatus::Running,
            JobStatus::Completed,
            JobStatus::Failed,
            JobStatus::Cancelled,
            JobStatus::Interrupted,
        ] {
            let s = status.to_string();
            let parsed: JobStatus = s.parse().unwrap();
            assert_eq!(status, parsed);
        }
    }

    #[test]
    fn job_status_predicates() {
        assert!(!JobStatus::Queued.is_terminal());
        assert!(!JobStatus::Running.is_terminal());
        assert!(JobStatus::Completed.is_terminal());
        assert!(JobStatus::Failed.is_terminal());
        assert!(JobStatus::Cancelled.is_terminal());
        assert!(JobStatus::Interrupted.is_terminal());

        assert!(JobStatus::Queued.is_active());
        assert!(JobStatus::Running.is_active());
        assert!(!JobStatus::Completed.is_active());

        assert!(JobStatus::Queued.can_cancel());
        assert!(JobStatus::Running.can_cancel());
        assert!(!JobStatus::Completed.can_cancel());

        assert!(!JobStatus::Queued.can_restart());
        assert!(JobStatus::Cancelled.can_restart());
        assert!(JobStatus::Failed.can_restart());
    }

    #[test]
    fn file_status_kind_predicates() {
        assert!(!FileStatusKind::Queued.is_terminal());
        assert!(!FileStatusKind::Processing.is_terminal());
        assert!(FileStatusKind::Done.is_terminal());
        assert!(FileStatusKind::Error.is_terminal());
        assert!(!FileStatusKind::Interrupted.is_terminal());

        assert!(FileStatusKind::Queued.is_resumable());
        assert!(FileStatusKind::Processing.is_resumable());
        assert!(!FileStatusKind::Done.is_resumable());
        assert!(!FileStatusKind::Error.is_resumable());
        assert!(FileStatusKind::Interrupted.is_resumable());
    }

    #[test]
    fn file_status_entry_roundtrip() {
        let entry = FileStatusEntry {
            filename: "test.cha".into(),
            status: FileStatusKind::Processing,
            error: None,
            error_category: None,
            error_codes: None,
            error_line: None,
            bug_report_id: None,
            started_at: Some(UnixTimestamp(1700000000.0)),
            finished_at: None,
            next_eligible_at: None,
            progress_current: Some(3),
            progress_total: Some(10),
            progress_stage: Some(FileProgressStage::AnalyzingMorphosyntax),
            progress_label: Some("Analyzing morphosyntax".into()),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let back: FileStatusEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry, back);
    }

    #[test]
    fn health_response_defaults() {
        let json = "{}";
        let health: HealthResponse = serde_json::from_str(json).unwrap();
        assert_eq!(health.status, HealthStatus::Ok);
        assert_eq!(health.cache_backend, "sqlite");
        assert!(!health.free_threaded);
        assert_eq!(health.workers_available, 0);
        assert_eq!(health.job_slots_available, 0);
        assert_eq!(health.live_workers, 0);
        assert!(health.live_worker_keys.is_empty());
    }

    #[test]
    fn job_info_full_roundtrip() {
        let info = JobInfo {
            job_id: "abc-123".into(),
            status: JobStatus::Running,
            command: ReleasedCommand::Morphotag,
            options: crate::options::CommandOptions::Morphotag(crate::options::MorphotagOptions {
                common: crate::options::CommonOptions::default(),

                ..Default::default()
            }),
            // Morphotag JobInfo always carries `LanguageSpec::PerFile` —
            // the dashboard and JSON API surface this as `"per-file"`.
            // Previously a job-level `eng` placeholder leaked here; that
            // is the bug this test now guards against.
            lang: LanguageSpec::PerFile,
            source_dir: "/data/corpus".into(),
            total_files: 10,
            completed_files: 3,
            current_file: Some("04DM.cha".into()),
            error: None,
            file_statuses: vec![FileStatusEntry {
                filename: "04DM.cha".into(),
                status: FileStatusKind::Processing,
                error: None,
                error_category: None,
                error_codes: None,
                error_line: None,
                bug_report_id: None,
                started_at: Some(UnixTimestamp(1700000000.0)),
                finished_at: None,
                next_eligible_at: None,
                progress_current: None,
                progress_total: None,
                progress_stage: None,
                progress_label: None,
            }],
            submitted_at: Some("2026-01-15T10:00:00Z".into()),
            submitted_by: Some("192.168.1.1".into()),
            submitted_by_name: Some("Lab-Mac-1".into()),
            completed_at: None,
            duration_s: None,
            next_eligible_at: None,
            num_workers: Some(4),
            active_lease: Some(crate::scheduling::LeaseRecord {
                leased_by_node: "node-123".into(),
                heartbeat_at: UnixTimestamp(1700000001.0),
                expires_at: UnixTimestamp(1700000301.0),
            }),
            batch_progress: None,
            control_plane: None,
            execution_plan: None,
            last_cancelled_at: None,
            last_cancelled_source: None,
            last_cancelled_host: None,
            last_cancelled_reason: None,
        };
        let json = serde_json::to_string_pretty(&info).unwrap();
        let back: JobInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(info, back);
    }
}
