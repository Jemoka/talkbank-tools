CREATE TABLE IF NOT EXISTS cache_meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS file_cache (
    path_hash TEXT NOT NULL,
    file_path TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    version TEXT NOT NULL,
    cached_at INTEGER NOT NULL,
    check_alignment INTEGER NOT NULL,
    is_valid INTEGER NOT NULL,
    roundtrip_tested INTEGER NOT NULL DEFAULT 0,
    roundtrip_passed INTEGER,
    parser_kind TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_cache_lookup
    ON file_cache(path_hash, version, check_alignment, parser_kind);
CREATE INDEX IF NOT EXISTS idx_file_path ON file_cache(file_path);
