-- SQLite treats NULL values as distinct for UNIQUE indexes, so the old
-- composite unique index did not enforce uniqueness for validation rows where
-- parser_kind IS NULL.
--
-- Deduplicate existing rows first, keeping the latest inserted row per key.
DELETE FROM file_cache
WHERE parser_kind IS NULL
  AND rowid NOT IN (
    SELECT MAX(rowid)
    FROM file_cache
    WHERE parser_kind IS NULL
    GROUP BY path_hash, version, check_alignment
  );

DELETE FROM file_cache
WHERE parser_kind IS NOT NULL
  AND rowid NOT IN (
    SELECT MAX(rowid)
    FROM file_cache
    WHERE parser_kind IS NOT NULL
    GROUP BY path_hash, version, check_alignment, parser_kind
  );

DROP INDEX IF EXISTS idx_cache_lookup;

CREATE UNIQUE INDEX IF NOT EXISTS idx_cache_lookup_validation
    ON file_cache(path_hash, version, check_alignment)
    WHERE parser_kind IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_cache_lookup_roundtrip
    ON file_cache(path_hash, version, check_alignment, parser_kind)
    WHERE parser_kind IS NOT NULL;
