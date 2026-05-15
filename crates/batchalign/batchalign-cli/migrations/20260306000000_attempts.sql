CREATE TABLE IF NOT EXISTS attempts (
    attempt_id       TEXT PRIMARY KEY,
    job_id           TEXT NOT NULL REFERENCES jobs(job_id) ON DELETE CASCADE,
    work_unit_id     TEXT NOT NULL,
    work_unit_kind   TEXT NOT NULL,
    attempt_number   INTEGER NOT NULL,
    started_at       REAL NOT NULL,
    finished_at      REAL,
    outcome          TEXT NOT NULL,
    failure_category TEXT,
    disposition      TEXT NOT NULL,
    worker_node_id   TEXT,
    worker_pid       INTEGER,
    UNIQUE(job_id, work_unit_id, attempt_number)
);

CREATE INDEX IF NOT EXISTS idx_attempts_job_id
    ON attempts(job_id);

CREATE INDEX IF NOT EXISTS idx_attempts_job_work_unit
    ON attempts(job_id, work_unit_id);
