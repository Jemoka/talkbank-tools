"""Historical-failure ordering for pytest (Phase E of the test-cost
revamp).

Reads the SQLite history written by Phase B's conftest hooks and
computes a per-test priority so the collection-modifyitems hook can
put likely-failing, fast-running tests at the front. Combined with
Phase A's interactive fail-fast, this produces failures within
seconds of running ``uv run pytest`` regardless of suite size.

Priority formula::

    fail_rate = fails / total_runs_last_N_days
    priority  = fail_rate / max(avg_duration_s, epsilon)

Why this shape:

* Fails fast-and-often → very high priority. The tests most likely
  to surface the next bug, amortized over the smallest wait.
* Fails slow-and-often → medium priority. Still worth running
  first, but a long-running test at the front can dominate
  perceived latency.
* Never fails recently → zero priority. Sort among themselves by
  collection order.
* No history at all → slightly-negative priority. A freshly-added
  test has no signal, so sort it AFTER the zero-fail-but-has-data
  bucket; running it alongside the healthy tail is the safe place.

Only reads ``framework = 'pytest'`` rows so Rust nextest data (when
it starts landing in the same DB) doesn't contaminate the Python
ordering.
"""

from __future__ import annotations

import sqlite3
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

# Minimum divisor in the priority formula so a zero-duration test
# (never ran in the call phase, or the history was seeded with 0)
# doesn't divide by zero.
_DURATION_EPSILON = 1e-3


@dataclass(frozen=True)
class HistoryStats:
    """Aggregated outcomes for a single test over the query window.

    Named ``HistoryStats`` (not ``TestStats``) because pytest
    auto-collects any top-level class whose name starts with
    ``Test``. Even with ``__test__ = False`` the collection layer
    still warns, and the rename avoids the class of warning entirely.
    """

    test_id: str
    fails: int
    total: int
    avg_duration_s: float

    @property
    def fail_rate(self) -> float:
        return self.fails / self.total if self.total > 0 else 0.0

    @property
    def priority(self) -> float:
        """Sort key. Higher = run earlier. Negative = "no history"."""
        if self.total <= 0:
            # Freshly-added test. Sort after zero-fail-has-history but
            # before never-observed-at-all (which don't appear in the
            # dict at all — see ``order_by_priority``).
            return -1.0
        denom = max(self.avg_duration_s, _DURATION_EPSILON)
        return self.fail_rate / denom


def load_stats(db_path: Path, since_ts: int) -> dict[str, HistoryStats]:
    """Aggregate pytest test runs in the window ``ts >= since_ts``.

    Returns ``{test_id: HistoryStats}``. An empty dict when the DB file
    does not exist — the first-ever run has nothing to read, and
    callers should fall back to collection order.
    """
    if not db_path.exists():
        return {}
    conn = sqlite3.connect(db_path)
    try:
        rows = conn.execute(
            """
            SELECT
                test_id,
                SUM(CASE WHEN outcome = 'failed' THEN 1 ELSE 0 END),
                COUNT(*),
                AVG(duration_s)
            FROM test_runs
            WHERE ts >= ? AND framework = 'pytest'
            GROUP BY test_id
            """,
            (since_ts,),
        ).fetchall()
    finally:
        conn.close()
    return {
        test_id: HistoryStats(
            test_id=test_id,
            fails=int(fails or 0),
            total=int(total or 0),
            avg_duration_s=float(avg_duration or 0.0),
        )
        for test_id, fails, total, avg_duration in rows
    }


def order_by_priority(
    test_ids: Iterable[str], stats: dict[str, HistoryStats]
) -> list[str]:
    """Return ``test_ids`` sorted by descending priority.

    Tests not in ``stats`` (no history rows) preserve their relative
    original order and land AFTER every test with history. This
    gives fresh tests a neutral position — they won't be front-loaded
    (no failure signal) and won't starve (they still run in the
    same invocation).

    Python's ``sorted`` is stable, so ties in priority — including
    the "no history" bucket — keep their original relative order.
    Callers get deterministic output for deterministic input.
    """
    id_list = list(test_ids)

    # Enumerate preserves the original index, which is the tie-breaker
    # for tests with equal priority or no history.
    def key(indexed: tuple[int, str]) -> tuple[float, int]:
        i, tid = indexed
        s = stats.get(tid)
        # The key is sorted ASCENDING — smaller = earlier in the run.
        # We want "highest priority first," so we negate priority.
        # No history matches HistoryStats(total=0).priority = -1.0, so its
        # negated key is +1.0 — larger than any real fail_rate-derived
        # key, which puts no-history tests at the end of the queue.
        negated_priority = -s.priority if s is not None else 1.0
        return (negated_priority, i)

    indexed = list(enumerate(id_list))
    indexed.sort(key=key)
    return [tid for _, tid in indexed]
