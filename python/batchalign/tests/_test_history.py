"""Per-test duration + outcome history store.

Phase B2 of the test-cost revamp. A thin SQLite wrapper the pytest
conftest feeds from ``pytest_runtest_logreport``; nextest can feed
it in a second pass by parsing its JSON message stream. One row per
test invocation — deliberately denormalized so queries for
historical-failure-ordering (Phase E) and top-N slowest tests
(Phase B3) stay one ``SELECT`` away.

Layered on top of ``scripts/test-bg.sh`` so background runs also
contribute data (see ``conftest.py::pytest_runtest_logreport`` for
the wiring).

Rationale for SQLite vs flat JSONL: we need range + aggregation
queries for Phase E ordering; SQLite is the simplest engine that
gives us ``WHERE ts > ? AND test_id = ?`` with an index. At ~1 KB
per row and a few thousand tests per day, the DB stays small for
years. If it ever grows past ~1 GB, ``DELETE FROM test_runs WHERE
ts < ?`` is a one-liner for rotation.
"""

from __future__ import annotations

import sqlite3
import time
from pathlib import Path
from typing import Literal

# SQLite outcome strings. Mirrors pytest's ``report.outcome`` values
# (``passed`` / ``failed`` / ``skipped``) plus common subclassed
# outcomes. Kept as a Literal (not Enum) so callers can pass pytest's
# string directly without translation.
Outcome = Literal[
    "passed",
    "failed",
    "skipped",
    "xfailed",
    "xpassed",
    "error",
]

Framework = Literal["pytest", "nextest"]


SCHEMA_DDL = """
CREATE TABLE IF NOT EXISTS test_runs (
    ts         INTEGER NOT NULL,
    test_id    TEXT    NOT NULL,
    outcome    TEXT    NOT NULL,
    duration_s REAL    NOT NULL,
    commit_sha TEXT,
    framework  TEXT    NOT NULL DEFAULT 'pytest'
);
CREATE INDEX IF NOT EXISTS idx_runs_test_id_ts ON test_runs(test_id, ts);
CREATE INDEX IF NOT EXISTS idx_runs_ts         ON test_runs(ts);
"""


class HistoryWriter:
    """SQLite-backed writer for per-test run history.

    Not thread-safe. Callers that want concurrent writes (e.g., xdist
    workers) should instantiate one writer per worker and point it at
    the same DB — SQLite handles the per-process locking via WAL mode.
    """

    def __init__(self, db_path: Path) -> None:
        db_path.parent.mkdir(parents=True, exist_ok=True)
        self._conn: sqlite3.Connection | None = sqlite3.connect(db_path)
        # WAL gives us concurrent reads during writes — critical for
        # live-tailing a test run from another process (dashboard,
        # watcher, etc.).
        self._conn.execute("PRAGMA journal_mode = WAL")
        self._conn.executescript(SCHEMA_DDL)

    def record(
        self,
        test_id: str,
        outcome: Outcome | str,
        duration_s: float,
        *,
        commit_sha: str | None = None,
        framework: Framework | str = "pytest",
    ) -> None:
        """Insert one row. Commits immediately — no batching.

        We commit per-row rather than batching because a crashing test
        process is exactly when we most want the prior runs' data to
        survive. SQLite's per-insert overhead at this write rate
        (hundreds per second max) is negligible.
        """
        if self._conn is None:
            raise sqlite3.ProgrammingError("record() after close()")
        self._conn.execute(
            "INSERT INTO test_runs "
            "(ts, test_id, outcome, duration_s, commit_sha, framework) "
            "VALUES (?, ?, ?, ?, ?, ?)",
            (int(time.time()), test_id, outcome, duration_s, commit_sha, framework),
        )
        self._conn.commit()

    def close(self) -> None:
        """Idempotent — safe to call more than once."""
        if self._conn is not None:
            self._conn.close()
            self._conn = None
