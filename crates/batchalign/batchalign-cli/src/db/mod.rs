//! SQLite persistence layer — port of `batchalign/serve/job_db.py`.
//!
//! Write-through SQLite database at the runtime-owned `jobs.db` path under the
//! resolved state root. Called by
//! `JobStore` at each state transition for crash recovery. Uses WAL mode
//! with `busy_timeout` for safe concurrent access.
//!
//! All DB operations are natively async via `sqlx::SqlitePool`.

mod insert;
mod query;
mod recovery;
mod schema;
mod update;

pub use insert::NewJobRecord;
pub use query::CancellationRow;
pub use schema::{AttemptRow, FileStatusRow, JobRow};

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};

use crate::config::RuntimeLayout;
use crate::error::ServerError;

// ---------------------------------------------------------------------------
// JobDB
// ---------------------------------------------------------------------------

/// Write-through SQLite layer for job persistence.
///
/// Thread-safe via `SqlitePool` (connection pooling).
pub struct JobDB {
    pool: SqlitePool,
    db_path: PathBuf,
}

impl JobDB {
    /// Open (or create) the job database and run migrations.
    pub async fn open(db_dir: Option<&Path>) -> Result<Self, ServerError> {
        let layout = RuntimeLayout::from_env();
        Self::open_with_layout(&layout, db_dir).await
    }

    /// Open the job database using an explicit runtime layout for the default
    /// state root.
    pub async fn open_with_layout(
        layout: &RuntimeLayout,
        db_dir: Option<&Path>,
    ) -> Result<Self, ServerError> {
        let db_dir = match db_dir {
            Some(d) => d.to_path_buf(),
            None => layout.state_dir().to_path_buf(),
        };
        std::fs::create_dir_all(&db_dir).map_err(|e| {
            ServerError::Io(std::io::Error::new(
                e.kind(),
                format!("creating db dir {}: {e}", db_dir.display()),
            ))
        })?;
        let db_path = db_dir.join("jobs.db");

        let options = SqliteConnectOptions::new()
            .filename(&db_path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(std::time::Duration::from_millis(10_000))
            .synchronous(SqliteSynchronous::Normal)
            .foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;

        sqlx::migrate!("./migrations").run(&pool).await?;

        Ok(Self { pool, db_path })
    }

    /// The path to the database file.
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }
}

fn unix_now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::UnixTimestamp;
    use crate::options::{
        AlignOptions, CommandOptions, CommonOptions, FaEngineName, MorphotagOptions,
    };
    use crate::scheduling::{AttemptOutcome, FailureCategory, RetryDisposition, WorkUnitKind};
    use crate::worker::WorkerPid;

    fn morphotag_options() -> CommandOptions {
        CommandOptions::Morphotag(MorphotagOptions {
            common: CommonOptions::default(),

            ..Default::default()
        })
    }

    fn align_options() -> CommandOptions {
        CommandOptions::Align(AlignOptions {
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
        })
    }

    /// Build a new queued job insert payload for DB tests.
    fn make_job_record(
        job_id: &str,
        command: &str,
        options: CommandOptions,
        filenames: Vec<String>,
        has_chat: Vec<bool>,
    ) -> NewJobRecord {
        NewJobRecord {
            job_id: job_id.to_string(),
            correlation_id: job_id.to_string(),
            command: command.to_string(),
            lang: "eng".to_string(),
            num_speakers: 1,
            status: "queued".to_string(),
            staging_dir: "/tmp/staging".to_string(),
            filenames,
            has_chat,
            options,
            media_mapping: String::new(),
            media_subdir: String::new(),
            source_dir: String::new(),
            submitted_by: "127.0.0.1".to_string(),
            submitted_by_name: "localhost".to_string(),
            submitted_at: 1700000000.0,
            paths_mode: false,
            source_paths: Vec::new(),
            output_paths: Vec::new(),
        }
    }

    async fn test_db() -> (JobDB, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let db = JobDB::open(Some(dir.path())).await.unwrap();
        (db, dir)
    }

    #[tokio::test]
    async fn schema_creation() {
        let dir = tempfile::tempdir().unwrap();
        let _db = JobDB::open(Some(dir.path())).await.unwrap();
        // Second open should be idempotent (migrations already applied)
        let _db2 = JobDB::open(Some(dir.path())).await.unwrap();
    }

    #[tokio::test]
    async fn insert_and_load_roundtrip() {
        let (db, _dir) = test_db().await;
        let record = make_job_record(
            "job1",
            "morphotag",
            morphotag_options(),
            vec!["test.cha".to_string()],
            vec![true],
        );

        db.insert_job(&record).await.unwrap();

        let jobs = db.load_all_jobs().await.unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].job_id, "job1");
        assert_eq!(jobs[0].correlation_id, "job1");
        assert_eq!(jobs[0].command, "morphotag");
        assert_eq!(jobs[0].filenames, vec!["test.cha"]);
        assert_eq!(jobs[0].file_statuses.len(), 1);
        assert_eq!(jobs[0].file_statuses[0].status, "queued");
    }

    #[tokio::test]
    async fn update_job_status() {
        let (db, _dir) = test_db().await;
        let record = make_job_record(
            "job1",
            "morphotag",
            morphotag_options(),
            vec!["f.cha".into()],
            vec![true],
        );

        db.insert_job(&record).await.unwrap();

        db.update_job_status("job1", "running", None, None, Some(4), None)
            .await
            .unwrap();
        let jobs = db.load_all_jobs().await.unwrap();
        assert_eq!(jobs[0].status, "running");
        assert_eq!(jobs[0].num_workers, Some(4));
    }

    #[tokio::test]
    async fn update_file_status() {
        let (db, _dir) = test_db().await;
        let mut record = make_job_record(
            "job1",
            "align",
            align_options(),
            vec!["a.cha".into()],
            vec![true],
        );
        record.status = "running".to_string();
        record.staging_dir = "/tmp".to_string();

        db.insert_job(&record).await.unwrap();

        db.update_file_status(
            "job1",
            "a.cha",
            "done",
            None,
            None,
            None,
            Some("chat"),
            Some(1700000001.0),
            Some(1700000005.0),
            None,
        )
        .await
        .unwrap();

        let jobs = db.load_all_jobs().await.unwrap();
        assert_eq!(jobs[0].file_statuses[0].status, "done");
        assert_eq!(jobs[0].file_statuses[0].content_type, "chat");
    }

    #[tokio::test]
    async fn attempt_roundtrip() {
        let (db, _dir) = test_db().await;
        let mut record = make_job_record(
            "job1",
            "morphotag",
            morphotag_options(),
            vec!["a.cha".into()],
            vec![true],
        );
        record.status = "running".to_string();
        record.staging_dir = "/tmp".to_string();

        db.insert_job(&record).await.unwrap();

        let (attempt_id, attempt_number) = db
            .insert_attempt_start(
                "job1",
                "a.cha",
                WorkUnitKind::FileProcess,
                1700000001.0,
                Some("node-a"),
                Some(4321),
            )
            .await
            .unwrap();
        assert_eq!(attempt_id, "job1:a.cha:1");
        assert_eq!(attempt_number, 1);

        db.finish_attempt(
            &attempt_id,
            AttemptOutcome::Failed,
            Some(FailureCategory::WorkerCrash),
            RetryDisposition::TerminalFailure,
            1700000002.5,
        )
        .await
        .unwrap();

        let attempts = db.load_attempts_for_job("job1").await.unwrap();
        assert_eq!(attempts.len(), 1);
        assert_eq!(&*attempts[0].attempt_id, attempt_id);
        assert_eq!(attempts[0].job_id, "job1");
        assert_eq!(attempts[0].work_unit_id, "a.cha");
        assert_eq!(attempts[0].work_unit_kind, WorkUnitKind::FileProcess);
        assert_eq!(attempts[0].attempt_number, 1);
        assert_eq!(attempts[0].outcome, AttemptOutcome::Failed);
        assert_eq!(
            attempts[0].failure_category,
            Some(FailureCategory::WorkerCrash)
        );
        assert_eq!(attempts[0].disposition, RetryDisposition::TerminalFailure);
        assert_eq!(attempts[0].worker_node_id.as_deref(), Some("node-a"));
        assert_eq!(attempts[0].worker_pid, Some(WorkerPid(4321)));
        assert_eq!(attempts[0].finished_at, Some(UnixTimestamp(1700000002.5)));
    }

    #[tokio::test]
    async fn cascade_delete() {
        let (db, _dir) = test_db().await;
        let mut record = make_job_record(
            "job1",
            "morphotag",
            morphotag_options(),
            vec!["a.cha".into(), "b.cha".into()],
            vec![true, true],
        );
        record.status = "completed".to_string();
        record.staging_dir = "/tmp".to_string();

        db.insert_job(&record).await.unwrap();

        db.delete_job(&crate::api::JobId::from("job1"))
            .await
            .unwrap();
        let jobs = db.load_all_jobs().await.unwrap();
        assert!(jobs.is_empty());
    }

    #[tokio::test]
    async fn recover_interrupted() {
        let (db, _dir) = test_db().await;

        // Insert a running job
        let mut running = make_job_record(
            "job1",
            "align",
            align_options(),
            vec!["a.cha".into()],
            vec![true],
        );
        running.status = "running".to_string();
        running.staging_dir = "/tmp".to_string();

        db.insert_job(&running).await.unwrap();
        db.update_file_status(
            "job1",
            "a.cha",
            "processing",
            None,
            None,
            None,
            None,
            Some(1700000001.0),
            None,
            None,
        )
        .await
        .unwrap();

        // Insert a queued job
        let mut queued = make_job_record(
            "job2",
            "morphotag",
            morphotag_options(),
            vec!["b.cha".into()],
            vec![true],
        );
        queued.staging_dir = "/tmp2".to_string();
        queued.submitted_at = 1700000002.0;

        db.insert_job(&queued).await.unwrap();

        // Insert a completed job (should NOT be interrupted)
        let mut completed = make_job_record(
            "job3",
            "morphotag",
            morphotag_options(),
            vec!["c.cha".into()],
            vec![true],
        );
        completed.status = "completed".to_string();
        completed.staging_dir = "/tmp3".to_string();
        completed.submitted_at = 1700000003.0;

        db.insert_job(&completed).await.unwrap();

        let interrupted = db.recover_interrupted().await.unwrap();
        assert_eq!(interrupted.len(), 2);
        assert!(interrupted.contains(&"job1".to_string()));
        assert!(interrupted.contains(&"job2".to_string()));

        let jobs = db.load_all_jobs().await.unwrap();
        for job in &jobs {
            if job.job_id == "job1" || job.job_id == "job2" {
                assert_eq!(job.status, "interrupted");
            }
            if job.job_id == "job3" {
                assert_eq!(job.status, "completed");
            }
        }
    }

    #[tokio::test]
    async fn prune_expired() {
        let (db, _dir) = test_db().await;

        // Insert an old job (submitted 10 days ago)
        let old_time = unix_now() - 10.0 * 86400.0;
        let mut old_job = make_job_record(
            "old",
            "morphotag",
            morphotag_options(),
            vec!["a.cha".into()],
            vec![true],
        );
        old_job.status = "completed".to_string();
        old_job.staging_dir = "/tmp/old".to_string();
        old_job.submitted_at = old_time;

        db.insert_job(&old_job).await.unwrap();

        // Insert a recent job
        let mut new_job = make_job_record(
            "new",
            "morphotag",
            morphotag_options(),
            vec!["b.cha".into()],
            vec![true],
        );
        new_job.status = "completed".to_string();
        new_job.staging_dir = "/tmp/new".to_string();
        new_job.submitted_at = unix_now();

        db.insert_job(&new_job).await.unwrap();

        let dirs = db.prune_expired(7).await.unwrap();
        assert_eq!(dirs.len(), 1);
        assert_eq!(dirs[0], "/tmp/old");

        let jobs = db.load_all_jobs().await.unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].job_id, "new");
    }

    #[tokio::test]
    async fn paths_mode_roundtrip() {
        let (db, _dir) = test_db().await;
        let mut record = make_job_record(
            "job1",
            "morphotag",
            morphotag_options(),
            vec!["a.cha".into()],
            vec![true],
        );
        record.staging_dir = "/tmp".to_string();
        record.source_dir = "/src".to_string();
        record.paths_mode = true;
        record.source_paths = vec!["/src/a.cha".into()];
        record.output_paths = vec!["/out/a.cha".into()];
        record.submitted_by_name = String::new();

        db.insert_job(&record).await.unwrap();

        let jobs = db.load_all_jobs().await.unwrap();
        assert!(jobs[0].paths_mode);
        assert_eq!(jobs[0].source_paths, vec!["/src/a.cha"]);
        assert_eq!(jobs[0].output_paths, vec!["/out/a.cha"]);
    }

    // -----------------------------------------------------------------------
    // T097: Recovery evidence preservation tests
    // -----------------------------------------------------------------------

    /// Recovery must NOT erase error messages on files that already failed before
    /// the server crash. A file that was in `"error"` state with a recorded error
    /// message must keep that evidence after `recover_interrupted()`.
    #[tokio::test]
    async fn recovery_preserves_completed_file_errors() {
        let (db, _dir) = test_db().await;

        // Insert a running job with 2 files: one already errored, one still processing.
        let mut job = make_job_record(
            "evidence-job",
            "morphotag",
            morphotag_options(),
            vec!["ok.cha".into(), "broken.cha".into()],
            vec![true, true],
        );
        job.status = "running".to_string();
        db.insert_job(&job).await.unwrap();

        // File "broken.cha" already failed before the crash.
        db.update_file_status(
            "evidence-job",
            "broken.cha",
            "error",
            Some("Stanza crashed: CUDA OOM"),
            Some("worker_crash"),
            None,
            None,
            Some(1700000001.0),
            Some(1700000002.0),
            None,
        )
        .await
        .unwrap();

        // File "ok.cha" was still processing when the server crashed.
        db.update_file_status(
            "evidence-job",
            "ok.cha",
            "processing",
            None,
            None,
            None,
            None,
            Some(1700000001.0),
            None,
            None,
        )
        .await
        .unwrap();

        // Simulate server restart — run recovery.
        let interrupted = db.recover_interrupted().await.unwrap();
        assert_eq!(interrupted, vec!["evidence-job"]);

        // Reload and verify evidence preservation.
        let jobs = db.load_all_jobs().await.unwrap();
        let job = &jobs[0];
        assert_eq!(job.status, "interrupted");

        // File "broken.cha" should STILL have its error message — recovery must
        // not overwrite "error" status files.
        let broken = job
            .file_statuses
            .iter()
            .find(|fs| fs.filename == "broken.cha")
            .expect("broken.cha should exist");
        assert_eq!(
            broken.error.as_deref(),
            Some("Stanza crashed: CUDA OOM"),
            "recovery must preserve pre-crash error messages"
        );
        assert_eq!(
            broken.error_category.as_deref(),
            Some("worker_crash"),
            "recovery must preserve error categories"
        );

        // File "ok.cha" should be marked interrupted (was still processing).
        let ok = job
            .file_statuses
            .iter()
            .find(|fs| fs.filename == "ok.cha")
            .expect("ok.cha should exist");
        assert_eq!(
            ok.status, "interrupted",
            "processing file should become interrupted after recovery"
        );
    }

    /// Recovery must preserve timing evidence (started_at, finished_at) on files
    /// that were already done or errored before the crash.
    #[tokio::test]
    async fn recovery_preserves_timing_evidence() {
        let (db, _dir) = test_db().await;

        let mut job = make_job_record(
            "timing-job",
            "align",
            align_options(),
            vec!["timed.cha".into(), "untimed.cha".into()],
            vec![true, true],
        );
        job.status = "running".to_string();
        db.insert_job(&job).await.unwrap();

        // "timed.cha" completed successfully with timing.
        db.update_file_status(
            "timing-job",
            "timed.cha",
            "done",
            None,
            None,
            None,
            None,
            Some(1700000010.0),
            Some(1700000020.0),
            None,
        )
        .await
        .unwrap();

        // "untimed.cha" was still queued.
        // (no update needed — default status is "queued")

        let interrupted = db.recover_interrupted().await.unwrap();
        assert_eq!(interrupted.len(), 1);

        let jobs = db.load_all_jobs().await.unwrap();
        let job = &jobs[0];

        // "timed.cha" should keep its timing and "done" status — recovery only
        // touches queued/processing files.
        let timed = job
            .file_statuses
            .iter()
            .find(|fs| fs.filename == "timed.cha")
            .expect("timed.cha should exist");
        assert_eq!(timed.status, "done", "completed file should stay done");
        assert_eq!(
            timed.started_at,
            Some(1700000010.0),
            "started_at must be preserved"
        );
        assert_eq!(
            timed.finished_at,
            Some(1700000020.0),
            "finished_at must be preserved"
        );

        // "untimed.cha" was queued → should become interrupted.
        let untimed = job
            .file_statuses
            .iter()
            .find(|fs| fs.filename == "untimed.cha")
            .expect("untimed.cha should exist");
        assert_eq!(
            untimed.status, "interrupted",
            "queued file should become interrupted"
        );
    }

    /// A job with mixed file states (done + error + processing + queued) should
    /// have recovery only touch the in-flight files, leaving completed and errored
    /// files exactly as they were.
    #[tokio::test]
    async fn recovery_mixed_file_states() {
        let (db, _dir) = test_db().await;

        let mut job = make_job_record(
            "mixed-job",
            "morphotag",
            morphotag_options(),
            vec![
                "done.cha".into(),
                "error.cha".into(),
                "processing.cha".into(),
                "queued.cha".into(),
            ],
            vec![true, true, true, true],
        );
        job.status = "running".to_string();
        db.insert_job(&job).await.unwrap();

        db.update_file_status(
            "mixed-job",
            "done.cha",
            "done",
            None,
            None,
            None,
            None,
            Some(1700000001.0),
            Some(1700000005.0),
            None,
        )
        .await
        .unwrap();
        db.update_file_status(
            "mixed-job",
            "error.cha",
            "error",
            Some("parse failed: missing @Begin"),
            Some("parse_error"),
            Some("bug-report-uuid-123"),
            None,
            Some(1700000002.0),
            Some(1700000003.0),
            None,
        )
        .await
        .unwrap();
        db.update_file_status(
            "mixed-job",
            "processing.cha",
            "processing",
            None,
            None,
            None,
            None,
            Some(1700000004.0),
            None,
            None,
        )
        .await
        .unwrap();
        // "queued.cha" stays in default queued state.

        let interrupted = db.recover_interrupted().await.unwrap();
        assert_eq!(interrupted, vec!["mixed-job"]);

        let jobs = db.load_all_jobs().await.unwrap();
        let job = &jobs[0];
        assert_eq!(job.status, "interrupted");

        // done.cha: untouched.
        let done = job
            .file_statuses
            .iter()
            .find(|fs| fs.filename == "done.cha")
            .unwrap();
        assert_eq!(done.status, "done");

        // error.cha: untouched — all evidence preserved.
        let errored = job
            .file_statuses
            .iter()
            .find(|fs| fs.filename == "error.cha")
            .unwrap();
        assert_eq!(errored.status, "error");
        assert_eq!(
            errored.error.as_deref(),
            Some("parse failed: missing @Begin")
        );
        assert_eq!(errored.error_category.as_deref(), Some("parse_error"));
        assert_eq!(
            errored.bug_report_id.as_deref(),
            Some("bug-report-uuid-123"),
            "bug report ID must survive recovery"
        );

        // processing.cha: marked interrupted.
        let processing = job
            .file_statuses
            .iter()
            .find(|fs| fs.filename == "processing.cha")
            .unwrap();
        assert_eq!(processing.status, "interrupted");

        // queued.cha: marked interrupted.
        let queued = job
            .file_statuses
            .iter()
            .find(|fs| fs.filename == "queued.cha")
            .unwrap();
        assert_eq!(queued.status, "interrupted");
    }
}
