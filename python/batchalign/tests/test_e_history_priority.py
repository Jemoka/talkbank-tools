# affects: batchalign/tests/_history_priority.py
# affects: batchalign/tests/conftest.py
# affects: batchalign/tests/_test_history.py
"""Unit tests for history-driven test ordering (Phase E of the
test-cost revamp).

The priority function turns the Phase B SQLite history into a
per-test sort key:

    fail_rate = fails / total_runs_last_N_days
    priority  = fail_rate / max(avg_duration_s, epsilon)

So a test that fails often AND runs fast goes to the front of the
queue, producing failures in seconds even on huge suites. A test
that never fails has priority 0; a test with no history has priority
-1 (runs last).

Tests here cover the math and the loader. The pytest-hook wiring is
integration-tested in ``test_e_history_plugin.py``.
"""

from __future__ import annotations

import sqlite3
import time
from pathlib import Path

from batchalign.tests._history_priority import (
    HistoryStats,
    load_stats,
    order_by_priority,
)
from batchalign.tests._test_history import SCHEMA_DDL


def _seed(db: Path, rows: list[tuple[int, str, str, float]]) -> None:
    """Write (ts, test_id, outcome, duration_s) rows into a fresh DB."""
    db.parent.mkdir(parents=True, exist_ok=True)
    conn = sqlite3.connect(db)
    try:
        conn.executescript(SCHEMA_DDL)
        conn.executemany(
            "INSERT INTO test_runs (ts, test_id, outcome, duration_s) "
            "VALUES (?, ?, ?, ?)",
            rows,
        )
        conn.commit()
    finally:
        conn.close()


# ---------- HistoryStats math -----------------------------------------------


def test_fail_rate_zero_when_no_total() -> None:
    s = HistoryStats(test_id="t", fails=0, total=0, avg_duration_s=1.0)
    assert s.fail_rate == 0.0
    # No history → priority is negative so the test sorts AFTER zero-fail
    # tests that do have history.
    assert s.priority < 0


def test_fail_rate_standard_case() -> None:
    s = HistoryStats(test_id="t", fails=3, total=10, avg_duration_s=2.0)
    assert s.fail_rate == 0.3
    assert s.priority == 0.3 / 2.0


def test_zero_duration_uses_epsilon() -> None:
    # A test with duration 0 (never ran in call phase) shouldn't divide
    # by zero; we clip to a tiny epsilon so the fail rate dominates.
    s = HistoryStats(test_id="t", fails=1, total=1, avg_duration_s=0.0)
    assert s.priority > 100  # effectively "top of the queue"


def test_zero_fails_with_history_has_zero_priority() -> None:
    s = HistoryStats(test_id="t", fails=0, total=5, avg_duration_s=1.0)
    assert s.priority == 0.0


# ---------- load_stats ---------------------------------------------------


def test_load_stats_aggregates_rows(tmp_path: Path) -> None:
    db = tmp_path / "h.sqlite"
    now = int(time.time())
    _seed(
        db,
        [
            (now - 10, "a::t1", "passed", 1.0),
            (now - 20, "a::t1", "failed", 1.5),
            (now - 30, "a::t1", "passed", 0.5),
            (now - 40, "a::t2", "passed", 2.0),
        ],
    )
    stats = load_stats(db, since_ts=now - 1000)
    assert set(stats) == {"a::t1", "a::t2"}
    t1 = stats["a::t1"]
    assert t1.total == 3
    assert t1.fails == 1
    assert abs(t1.avg_duration_s - 1.0) < 0.01  # (1.0 + 1.5 + 0.5) / 3
    t2 = stats["a::t2"]
    assert t2.total == 1
    assert t2.fails == 0


def test_load_stats_respects_since_window(tmp_path: Path) -> None:
    db = tmp_path / "h.sqlite"
    now = int(time.time())
    _seed(
        db,
        [
            (now - 10_000, "old::t", "failed", 1.0),
            (now - 10, "new::t", "passed", 1.0),
        ],
    )
    stats = load_stats(db, since_ts=now - 100)
    assert "old::t" not in stats
    assert "new::t" in stats


def test_load_stats_missing_db_returns_empty(tmp_path: Path) -> None:
    # Not an error — the first-ever run has no DB yet. Ordering falls
    # back to collection order.
    missing = tmp_path / "does-not-exist.sqlite"
    assert load_stats(missing, since_ts=0) == {}


def test_load_stats_non_pytest_rows_excluded(tmp_path: Path) -> None:
    """Nextest-sourced rows share the DB but have different test_id
    conventions. load_stats filters to framework='pytest' by default
    so pytest ordering doesn't accidentally consume Rust data."""
    db = tmp_path / "h.sqlite"
    now = int(time.time())
    conn = sqlite3.connect(db)
    try:
        conn.executescript(
            """
            CREATE TABLE test_runs (
                ts INTEGER, test_id TEXT, outcome TEXT,
                duration_s REAL, commit_sha TEXT, framework TEXT
            );
            """
        )
        conn.executemany(
            "INSERT INTO test_runs VALUES (?, ?, ?, ?, ?, ?)",
            [
                (now, "py::t", "passed", 1.0, None, "pytest"),
                (now, "rust::t", "passed", 1.0, None, "nextest"),
            ],
        )
        conn.commit()
    finally:
        conn.close()

    stats = load_stats(db, since_ts=now - 100)
    assert "py::t" in stats
    assert "rust::t" not in stats


# ---------- order_by_priority --------------------------------------------


def test_order_by_priority_fails_first() -> None:
    stats = {
        "a::slow_but_reliable": HistoryStats("a::slow_but_reliable", fails=0, total=10, avg_duration_s=5.0),
        "b::fast_flaky": HistoryStats("b::fast_flaky", fails=3, total=10, avg_duration_s=0.1),
        "c::slow_flaky": HistoryStats("c::slow_flaky", fails=3, total=10, avg_duration_s=2.0),
    }
    ordered = order_by_priority(
        ["a::slow_but_reliable", "b::fast_flaky", "c::slow_flaky"],
        stats,
    )
    # fast_flaky first (0.3/0.1 = 3.0), slow_flaky next (0.3/2.0 = 0.15),
    # slow_but_reliable last (0.0).
    assert ordered == ["b::fast_flaky", "c::slow_flaky", "a::slow_but_reliable"]


def test_order_by_priority_untracked_go_last() -> None:
    stats = {
        "flaky": HistoryStats("flaky", fails=1, total=2, avg_duration_s=1.0),
    }
    ordered = order_by_priority(["new_test", "flaky", "also_new"], stats)
    assert ordered[0] == "flaky"
    # Unknown tests preserve their original relative order.
    assert ordered[1:] == ["new_test", "also_new"]


def test_order_by_priority_stable_on_ties() -> None:
    # Two tests with identical priority keep their original relative order.
    stats = {
        "a": HistoryStats("a", fails=1, total=10, avg_duration_s=1.0),
        "b": HistoryStats("b", fails=1, total=10, avg_duration_s=1.0),
    }
    ordered = order_by_priority(["a", "b"], stats)
    assert ordered == ["a", "b"]
    ordered = order_by_priority(["b", "a"], stats)
    assert ordered == ["b", "a"]
