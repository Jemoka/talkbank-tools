//! Crash recovery — reload persisted jobs from SQLite on startup.

use std::collections::HashMap;

use crate::api::{
    ContentType, DisplayPath, FileStatusKind, JobId, JobStatus, NumSpeakers, ReleasedCommand,
    UnixTimestamp,
};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use super::super::job::{
    Job, JobDispatchConfig, JobExecutionState, JobFilesystemConfig, JobIdentity, JobLeaseState,
    JobRuntimeControl, JobScheduleState, JobSourceContext, RecoveryDisposition,
};
use super::super::{FileResultEntry, FileStatus, JobStore};
use crate::error::ServerError;

/// Persisted startup-recovery update for one job.
#[derive(Debug, Clone)]
struct RecoveredJobPersistence {
    /// Job whose persisted status needs canonicalization.
    job_id: JobId,
    /// Canonical status after recovery reconciliation.
    status: JobStatus,
    /// Terminal timestamp that should remain on the job row.
    completed_at: Option<UnixTimestamp>,
    /// Deferred retry deadline after recovery, if any.
    next_eligible_at: Option<UnixTimestamp>,
    /// Files that must be reset back to clean queued state in SQLite.
    requeued_files: Vec<String>,
}

fn append_recovery_note(existing: Option<String>, note: impl Into<String>) -> Option<String> {
    let note = format!("[recovery] {}", note.into());
    match existing {
        Some(existing) if existing.trim().is_empty() => Some(note),
        Some(existing) => Some(format!("{existing}\n{note}")),
        None => Some(note),
    }
}

fn recover_job_status(job_id: &str, raw_status: &str) -> (JobStatus, Option<String>) {
    match raw_status.parse() {
        Ok(status) => (status, None),
        Err(error) => {
            warn!(
                job_id,
                raw_status,
                %error,
                "Invalid persisted job status during crash recovery",
            );
            (
                JobStatus::Failed,
                Some(format!(
                    "invalid persisted job status '{raw_status}' was coerced to 'failed'"
                )),
            )
        }
    }
}

fn recover_file_status(
    job_id: &str,
    filename: &str,
    raw_status: &str,
) -> (FileStatusKind, Option<String>) {
    match raw_status.parse() {
        Ok(status) => (status, None),
        Err(error) => {
            warn!(
                job_id,
                filename,
                raw_status,
                %error,
                "Invalid persisted file status during crash recovery",
            );
            (
                FileStatusKind::Error,
                Some(format!(
                    "invalid persisted file status '{raw_status}' was coerced to 'error'"
                )),
            )
        }
    }
}

fn recover_failure_category(
    job_id: &str,
    filename: &str,
    raw_category: Option<&str>,
) -> (Option<crate::scheduling::FailureCategory>, Option<String>) {
    let Some(raw_category) = raw_category else {
        return (None, None);
    };

    match raw_category.parse() {
        Ok(category) => (Some(category), None),
        Err(error) => {
            warn!(
                job_id,
                filename,
                raw_category,
                %error,
                "Invalid persisted failure category during crash recovery",
            );
            (
                None,
                Some(format!(
                    "invalid persisted error_category '{raw_category}' was ignored"
                )),
            )
        }
    }
}

impl JobStore {
    /// Load jobs from DB into memory (crash recovery).
    pub async fn load_from_db(&self) -> Result<usize, ServerError> {
        let db = match &self.db {
            Some(db) => db,
            None => return Ok(0),
        };

        let rows = db.load_all_jobs().await?;
        let ttl_cutoff = super::super::unix_now().0 - (self.config.job_ttl_days as f64 * 86400.0);
        let (loaded, recovered_updates) = self
            .registry
            .mutate_all(move |jobs| {
                let mut loaded = 0;
                let mut recovered_updates = Vec::new();

                for row in rows {
                    if row.submitted_at < ttl_cutoff {
                        continue;
                    }

                    let (status, job_status_note) = recover_job_status(&row.job_id, &row.status);

                    let mut file_statuses = HashMap::new();
                    let mut results: Vec<FileResultEntry> = Vec::new();
                    for fs_row in &row.file_statuses {
                        let (fs_status, status_note) =
                            recover_file_status(&row.job_id, &fs_row.filename, &fs_row.status);
                        let (error_category, category_note) = recover_failure_category(
                            &row.job_id,
                            &fs_row.filename,
                            fs_row.error_category.as_deref(),
                        );
                        let mut file_error = fs_row.error.clone();
                        if let Some(note) = status_note {
                            file_error = append_recovery_note(file_error, note);
                        }
                        if let Some(note) = category_note {
                            file_error = append_recovery_note(file_error, note);
                        }

                        file_statuses.insert(
                            fs_row.filename.clone(),
                            FileStatus {
                                filename: DisplayPath::from(fs_row.filename.clone()),
                                status: fs_status,
                                error: file_error.clone(),
                                error_category,
                                error_codes: None,
                                error_line: None,
                                bug_report_id: fs_row.bug_report_id.clone(),
                                started_at: fs_row.started_at.map(UnixTimestamp),
                                finished_at: fs_row.finished_at.map(UnixTimestamp),
                                next_eligible_at: fs_row.next_eligible_at.map(UnixTimestamp),
                                current_attempt_id: None,
                                progress_current: None,
                                progress_total: None,
                                progress_stage: None,
                            },
                        );

                        if fs_status.is_terminal() {
                            results.push(FileResultEntry {
                                filename: DisplayPath::from(fs_row.filename.clone()),
                                content_type: match fs_row.content_type.as_str() {
                                    "csv" => ContentType::Csv,
                                    "text" => ContentType::Text,
                                    _ => ContentType::Chat,
                                },
                                error: file_error,
                            });
                        }
                    }

                    let completed_files = file_statuses
                        .values()
                        .filter(|file_status| file_status.status.is_terminal())
                        .count() as i64;

                    let job_id_newtype = JobId::from(row.job_id.clone());
                    let mut job_error = row.error.clone();
                    if let Some(note) = job_status_note {
                        job_error = append_recovery_note(job_error, note);
                    }
                    let mut job = Job {
                        identity: JobIdentity {
                            job_id: job_id_newtype.clone(),
                            correlation_id: if row.correlation_id.is_empty() {
                                row.job_id.clone().into()
                            } else {
                                row.correlation_id.into()
                            },
                        },
                        dispatch: JobDispatchConfig {
                            command: match ReleasedCommand::try_from(row.command.as_str()) {
                                Ok(cmd) => cmd,
                                Err(_) => {
                                    tracing::warn!(
                                        job_id = %row.job_id,
                                        command = %row.command,
                                        "Unknown command in DB, skipping job recovery"
                                    );
                                    continue;
                                }
                            },
                            lang: {
                                let (spec, valid) =
                                    crate::api::LanguageSpec::parse_from_db(&row.lang);
                                if !valid {
                                    tracing::warn!(
                                        job_id = %row.job_id,
                                        raw_lang = %row.lang,
                                        "Invalid language code in DB, falling back to eng"
                                    );
                                }
                                spec
                            },
                            num_speakers: NumSpeakers(row.num_speakers),
                            options: row.options,
                            runtime_state: std::collections::BTreeMap::new(),
                            debug_traces: false,
                        },
                        source: JobSourceContext {
                            submitted_by: row.submitted_by,
                            submitted_by_name: row.submitted_by_name,
                            source_dir: row.source_dir.into(),
                        },
                        filesystem: JobFilesystemConfig {
                            filenames: row.filenames.into_iter().map(DisplayPath::from).collect(),
                            has_chat: row.has_chat,
                            staging_dir: batchalign_types::paths::ServerPath::from(row.staging_dir),
                            paths_mode: row.paths_mode,
                            source_paths: row
                                .source_paths
                                .into_iter()
                                .map(batchalign_types::paths::ClientPath::from)
                                .collect(),
                            output_paths: row
                                .output_paths
                                .into_iter()
                                .map(batchalign_types::paths::ClientPath::from)
                                .collect(),
                            before_paths: Vec::new(),
                            media_mapping: batchalign_types::paths::MediaMappingKey::from(
                                row.media_mapping,
                            ),
                            media_subdir: batchalign_types::paths::RepoRelativePath::from(
                                row.media_subdir,
                            ),
                            // source_dir is owned by JobSourceContext; the runner snapshot
                            // assembles RunnerFilesystemConfig.source_dir from there.
                            source_dir: Default::default(),
                        },
                        execution: JobExecutionState {
                            status,
                            file_statuses,
                            results,
                            error: job_error,
                            completed_files,
                            batch_progress: None,
                        },
                        schedule: JobScheduleState {
                            submitted_at: UnixTimestamp(row.submitted_at),
                            completed_at: row.completed_at.map(UnixTimestamp),
                            next_eligible_at: row.next_eligible_at.map(UnixTimestamp),
                            num_workers: row.num_workers.map(|n| n as i64),
                            lease: JobLeaseState {
                                leased_by_node: row.leased_by_node.map(|node| node.into()),
                                expires_at: row.lease_expires_at.map(UnixTimestamp),
                                heartbeat_at: row.lease_heartbeat_at.map(UnixTimestamp),
                            },
                            last_cancel: row.last_cancelled_at.map(|at| {
                                crate::store::JobLastCancelInfo {
                                    at: UnixTimestamp(at),
                                    source: row
                                        .last_cancelled_source
                                        .clone()
                                        .unwrap_or_else(|| "api".to_string()),
                                    host: row.last_cancelled_host.clone(),
                                    reason: row.last_cancelled_reason.clone(),
                                }
                            }),
                        },
                        runtime: JobRuntimeControl {
                            cancel_token: CancellationToken::new(),
                            runner_active: false,
                        },
                        execution_plan: None,
                    };

                    if matches!(status, JobStatus::Interrupted | JobStatus::Running) {
                        let requeued_files = job
                            .execution
                            .file_statuses
                            .iter()
                            .filter(|(_, file_status)| file_status.status.is_resumable())
                            .map(|(filename, _)| filename.clone())
                            .collect();
                        let disposition = job.reconcile_recovered_runtime_state();
                        recovered_updates.push(RecoveredJobPersistence {
                            job_id: job_id_newtype.clone(),
                            status: job.execution.status,
                            completed_at: job.schedule.completed_at,
                            next_eligible_at: job.schedule.next_eligible_at,
                            requeued_files: match disposition {
                                RecoveryDisposition::Requeued => requeued_files,
                                RecoveryDisposition::Failed | RecoveryDisposition::Completed => {
                                    Vec::new()
                                }
                            },
                        });
                    }

                    jobs.insert(job_id_newtype, job);
                    loaded += 1;
                }

                (loaded, recovered_updates)
            })
            .await;

        for update in recovered_updates {
            db.update_job_status(
                update.job_id.as_ref(),
                &update.status.to_string(),
                None,
                update.completed_at.map(|timestamp| timestamp.0),
                None,
                update.next_eligible_at.map(|timestamp| timestamp.0),
            )
            .await?;
            db.update_job_lease(update.job_id.as_ref(), None, None, None)
                .await?;

            for filename in update.requeued_files {
                db.reset_recovered_file_to_queued(update.job_id.as_ref(), &filename)
                    .await?;
            }
        }

        info!(loaded = loaded, "Loaded jobs from DB");
        Ok(loaded)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tokio::sync::broadcast;

    use super::*;
    use crate::config::ServerConfig;
    use crate::db::{JobDB, NewJobRecord};
    use crate::options::{CommandOptions, CommonOptions, MorphotagOptions};
    use crate::store::{JobStore, unix_now};
    use crate::ws::BROADCAST_CAPACITY;

    /// Build a test insert payload for startup-recovery coverage.
    fn make_job_record(job_id: &str, status: &str, filenames: Vec<String>) -> NewJobRecord {
        let has_chat = filenames.iter().map(|_| true).collect();

        NewJobRecord {
            job_id: job_id.to_string(),
            correlation_id: job_id.to_string(),
            command: "morphotag".to_string(),
            lang: "eng".to_string(),
            num_speakers: 1,
            status: status.to_string(),
            staging_dir: "/tmp/staging".to_string(),
            filenames,
            has_chat,
            options: CommandOptions::Morphotag(MorphotagOptions {
                common: CommonOptions::default(),

                ..Default::default()
            }),
            media_mapping: String::new(),
            media_subdir: String::new(),
            source_dir: "/corpus".to_string(),
            submitted_by: "127.0.0.1".to_string(),
            submitted_by_name: "localhost".to_string(),
            submitted_at: unix_now().0,
            paths_mode: false,
            source_paths: Vec::new(),
            output_paths: Vec::new(),
        }
    }

    /// Open an isolated SQLite DB and `JobStore` pair for recovery tests.
    async fn test_store_with_db() -> (JobStore, Arc<JobDB>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let db = Arc::new(JobDB::open(Some(dir.path())).await.unwrap());
        let (tx, _rx) = broadcast::channel(BROADCAST_CAPACITY);
        let store = JobStore::new(ServerConfig::default(), Some(db.clone()), tx);
        (store, db, dir)
    }

    /// Startup recovery re-queues resumable work and persists the queued state.
    #[tokio::test]
    async fn load_from_db_requeues_resumable_interrupted_jobs() {
        let (store, db, _dir) = test_store_with_db().await;
        db.insert_job(&make_job_record(
            "job-1",
            "running",
            vec!["a.cha".into(), "b.cha".into()],
        ))
        .await
        .unwrap();
        db.update_file_status(
            "job-1",
            "a.cha",
            "done",
            None,
            None,
            None,
            Some("chat"),
            Some(10.0),
            Some(20.0),
            None,
        )
        .await
        .unwrap();
        db.update_file_status(
            "job-1",
            "b.cha",
            "processing",
            None,
            None,
            None,
            None,
            Some(15.0),
            None,
            None,
        )
        .await
        .unwrap();
        db.recover_interrupted().await.unwrap();

        store.load_from_db().await.unwrap();

        let info = store.get(&JobId::from("job-1")).await.unwrap();
        assert_eq!(info.status, JobStatus::Queued);
        let requeued_file = info
            .file_statuses
            .iter()
            .find(|file| file.filename == "b.cha")
            .unwrap();
        assert_eq!(requeued_file.status, FileStatusKind::Queued);
        assert!(requeued_file.started_at.is_none());

        let rows = db.load_all_jobs().await.unwrap();
        assert_eq!(rows[0].status, "queued");
        let persisted_file = rows[0]
            .file_statuses
            .iter()
            .find(|file| file.filename == "b.cha")
            .unwrap();
        assert_eq!(persisted_file.status, "queued");
        assert!(persisted_file.started_at.is_none());
    }

    /// Startup recovery finalizes all-terminal interrupted jobs and clears leases.
    #[tokio::test]
    async fn load_from_db_finalizes_terminal_interrupted_jobs() {
        let (store, db, _dir) = test_store_with_db().await;
        db.insert_job(&make_job_record(
            "job-2",
            "running",
            vec!["a.cha".into(), "b.cha".into()],
        ))
        .await
        .unwrap();
        db.update_file_status(
            "job-2",
            "a.cha",
            "done",
            None,
            None,
            None,
            Some("chat"),
            Some(10.0),
            Some(20.0),
            None,
        )
        .await
        .unwrap();
        db.update_file_status(
            "job-2",
            "b.cha",
            "error",
            Some("boom"),
            Some("worker_crash"),
            None,
            None,
            Some(11.0),
            Some(21.0),
            None,
        )
        .await
        .unwrap();
        db.update_job_lease("job-2", Some("node-a"), Some(40.0), Some(35.0))
            .await
            .unwrap();
        db.recover_interrupted().await.unwrap();

        store.load_from_db().await.unwrap();

        let info = store.get(&JobId::from("job-2")).await.unwrap();
        assert_eq!(info.status, JobStatus::Completed);
        assert_eq!(info.completed_files, 2);
        assert!(info.active_lease.is_none());

        let rows = db.load_all_jobs().await.unwrap();
        assert_eq!(rows[0].status, "completed");
        assert!(rows[0].lease_expires_at.is_none());
        assert!(rows[0].lease_heartbeat_at.is_none());
    }

    /// Recovery preserves invalid persisted job status evidence instead of silently dropping it.
    #[tokio::test]
    async fn load_from_db_preserves_invalid_job_status_evidence() {
        let (store, db, _dir) = test_store_with_db().await;
        db.insert_job(&make_job_record(
            "job-bad-status",
            "mystery_status",
            vec!["a.cha".into()],
        ))
        .await
        .unwrap();

        store.load_from_db().await.unwrap();

        let info = store.get(&JobId::from("job-bad-status")).await.unwrap();
        assert_eq!(info.status, JobStatus::Failed);
        let error = info
            .error
            .expect("recovery should preserve invalid status evidence");
        assert!(error.contains("invalid persisted job status 'mystery_status'"));
    }

    /// Recovery preserves invalid per-file persistence evidence instead of silently normalizing it.
    #[tokio::test]
    async fn load_from_db_preserves_invalid_file_persistence_evidence() {
        let (store, db, _dir) = test_store_with_db().await;
        db.insert_job(&make_job_record(
            "job-bad-file-status",
            "queued",
            vec!["bad.cha".into()],
        ))
        .await
        .unwrap();
        db.update_file_status(
            "job-bad-file-status",
            "bad.cha",
            "mystery_file_status",
            Some("original failure"),
            Some("mystery_category"),
            None,
            None,
            Some(10.0),
            Some(11.0),
            None,
        )
        .await
        .unwrap();

        store.load_from_db().await.unwrap();

        let info = store
            .get(&JobId::from("job-bad-file-status"))
            .await
            .unwrap();
        let file = info
            .file_statuses
            .iter()
            .find(|file| file.filename == "bad.cha")
            .expect("recovered file status");
        assert_eq!(file.status, FileStatusKind::Error);
        assert!(file.error_category.is_none());
        let error = file
            .error
            .clone()
            .expect("recovery should preserve invalid file persistence evidence");
        assert!(error.contains("original failure"));
        assert!(error.contains("invalid persisted file status 'mystery_file_status'"));
        assert!(error.contains("invalid persisted error_category 'mystery_category'"));
    }
}
