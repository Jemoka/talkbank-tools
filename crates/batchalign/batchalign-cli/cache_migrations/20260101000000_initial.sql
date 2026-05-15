CREATE TABLE IF NOT EXISTS cache_entries (
    key TEXT PRIMARY KEY,
    task TEXT NOT NULL,
    engine_version TEXT NOT NULL,
    batchalign_version TEXT NOT NULL,
    created_at TEXT NOT NULL,
    data BLOB NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_task ON cache_entries(task);
CREATE INDEX IF NOT EXISTS idx_created ON cache_entries(created_at);
