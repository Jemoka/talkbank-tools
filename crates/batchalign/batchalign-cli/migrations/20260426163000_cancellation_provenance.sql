-- Cancellation provenance: who cancelled a job, when, why, from where.
--
-- Two surfaces:
--   1. `cancellations` audit table — one row per cancel attempt, including
--      cancels against already-terminal jobs (recorded with accepted=0). This
--      is the forensic source of truth and captures multi-cancel patterns
--      (e.g., a user pressing cancel twice an hour apart when nothing
--      visibly happens in the TUI).
--   2. `jobs.last_cancelled_*` denormalized columns — most recent cancel
--      attempt's metadata projected onto the jobs row for fast list-view
--      display without a JOIN. Maintained as a write-through projection of
--      the audit table in the same transaction.
--
-- Schema rationale: see the cancellation-hygiene plan in the
-- private workspace (separate repo) Phase 1 § Schema change.

CREATE TABLE IF NOT EXISTS cancellations (
    id                 INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id             TEXT NOT NULL REFERENCES jobs(job_id) ON DELETE CASCADE,
    requested_at       REAL NOT NULL,
    source             TEXT NOT NULL,
    host               TEXT,
    pid                INTEGER,
    reason             TEXT,
    correlation_id     TEXT,
    in_flight_filename TEXT,
    accepted           INTEGER NOT NULL DEFAULT 1
);

-- Composite covers both the WHERE job_id=? lookup and the
-- ORDER BY requested_at ASC sort used by `list_cancellations` in
-- `crates/batchalign-app/src/db/query.rs`. SQLite can satisfy both
-- predicates from the same B-tree, no separate sort step needed.
CREATE INDEX IF NOT EXISTS idx_cancellations_job_id_requested_at
    ON cancellations(job_id, requested_at);

ALTER TABLE jobs ADD COLUMN last_cancelled_at     REAL;
ALTER TABLE jobs ADD COLUMN last_cancelled_source TEXT;
ALTER TABLE jobs ADD COLUMN last_cancelled_host   TEXT;
ALTER TABLE jobs ADD COLUMN last_cancelled_reason TEXT;
