CREATE TABLE IF NOT EXISTS jobs (
    job_id        TEXT PRIMARY KEY,
    command       TEXT NOT NULL,
    lang          TEXT NOT NULL,
    num_speakers  INTEGER NOT NULL DEFAULT 1,
    status        TEXT NOT NULL DEFAULT 'queued',
    error         TEXT,
    staging_dir   TEXT NOT NULL,
    filenames     TEXT NOT NULL,
    has_chat      TEXT NOT NULL,
    options       TEXT NOT NULL DEFAULT '{}',
    engine_overrides TEXT NOT NULL DEFAULT '{}',
    media_mapping TEXT NOT NULL DEFAULT '',
    media_subdir  TEXT NOT NULL DEFAULT '',
    source_dir    TEXT NOT NULL DEFAULT '',
    submitted_by  TEXT NOT NULL DEFAULT '',
    submitted_by_name TEXT NOT NULL DEFAULT '',
    submitted_at  REAL NOT NULL,
    completed_at  REAL,
    num_workers   INTEGER,
    paths_mode    INTEGER NOT NULL DEFAULT 0,
    source_paths  TEXT NOT NULL DEFAULT '[]',
    output_paths  TEXT NOT NULL DEFAULT '[]',
    correlation_id TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS file_statuses (
    job_id      TEXT NOT NULL REFERENCES jobs(job_id) ON DELETE CASCADE,
    filename    TEXT NOT NULL,
    status      TEXT NOT NULL DEFAULT 'queued',
    error       TEXT,
    error_category TEXT,
    bug_report_id TEXT,
    content_type TEXT NOT NULL DEFAULT 'chat',
    started_at  REAL,
    finished_at REAL,
    PRIMARY KEY (job_id, filename)
);
