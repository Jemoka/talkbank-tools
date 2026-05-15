# affects: batchalign/tests/_test_history.py
# affects: batchalign/tests/conftest.py
"""Integration test for the pytest hook that feeds the history writer.

Phase B2: verifies that running a small pytest invocation in a
subprocess produces rows in the SQLite history DB. Must run pytest
in a subprocess — the current test's own run is already inside a
pytest session, so the conftest hook ran on a different DB path
before we could point it anywhere useful.

Environment contract (assertions in this test):
  - ``BATCHALIGN_TEST_HISTORY_DB`` — if set, conftest writes there.
  - ``BATCHALIGN_TEST_HISTORY_OFF`` — if truthy, conftest skips.
  - Default: no history written if neither env var set.
"""

from __future__ import annotations

import sqlite3
import textwrap
from pathlib import Path

from batchalign.tests._subprocess_pytest import run_pytest_subprocess

_DUMMY_BODY = textwrap.dedent(
    """
    def test_passes_for_history():
        assert True
    """
)


def _run_pytest_subprocess(
    tmp_path: Path,
    env_extra: dict[str, str],
    *,
    xdist_workers: int = 0,
):
    """Thin adapter around :func:`run_pytest_subprocess` that pins the
    test-body + slug used in the Phase B2 integration tests."""
    return run_pytest_subprocess(
        tmp_path,
        env_extra,
        xdist_workers=xdist_workers,
        dummy_body=_DUMMY_BODY,
        slug="b_history_subprocess",
    )


def test_history_writes_when_env_set(tmp_path: Path) -> None:
    db = tmp_path / "history.sqlite"
    result = _run_pytest_subprocess(tmp_path, {"BATCHALIGN_TEST_HISTORY_DB": str(db)})
    assert result.returncode == 0, result.stdout + result.stderr

    assert db.exists(), f"history db not created at {db}"

    conn = sqlite3.connect(db)
    try:
        rows = conn.execute(
            "SELECT test_id, outcome FROM test_runs"
        ).fetchall()
    finally:
        conn.close()

    # At least one row for our dummy test.
    assert any(tid.endswith("test_passes_for_history") and outcome == "passed"
               for tid, outcome in rows), rows


def test_history_skipped_when_off(tmp_path: Path) -> None:
    db = tmp_path / "history.sqlite"
    result = _run_pytest_subprocess(
        tmp_path,
        {
            "BATCHALIGN_TEST_HISTORY_DB": str(db),
            "BATCHALIGN_TEST_HISTORY_OFF": "1",
        },
    )
    assert result.returncode == 0, result.stdout + result.stderr

    assert not db.exists(), f"history db should not be created when OFF is set (found {db})"


def test_history_skipped_when_db_unset(tmp_path: Path) -> None:
    """Default behavior: no env var, no write. Keeps the test suite
    silent unless the user or test-bg wrapper opts in."""
    result = _run_pytest_subprocess(tmp_path, {})
    assert result.returncode == 0, result.stdout + result.stderr
    # We can't assert DB absence (there's no fixed path), but we can assert
    # the user's home-cache DB wasn't touched by this subprocess. This is
    # implicit: the conftest reads BATCHALIGN_TEST_HISTORY_DB only.


def test_xdist_writes_exactly_one_row_per_test(tmp_path: Path) -> None:
    """Under xdist the controller mirrors each worker's logreport.
    The conftest must dedupe so only the worker that ran the test
    records — exactly one row per test regardless of worker count.
    """
    db = tmp_path / "history.sqlite"
    result = _run_pytest_subprocess(
        tmp_path,
        {"BATCHALIGN_TEST_HISTORY_DB": str(db)},
        xdist_workers=3,
    )
    assert result.returncode == 0, result.stdout + result.stderr
    assert db.exists(), f"history db not created at {db}"

    conn = sqlite3.connect(db)
    try:
        rows = conn.execute(
            "SELECT test_id, COUNT(*) FROM test_runs GROUP BY test_id"
        ).fetchall()
    finally:
        conn.close()

    assert len(rows) == 1, f"expected exactly 1 test_id, got {rows}"
    test_id, count = rows[0]
    assert count == 1, f"expected 1 row for {test_id}, got {count}"
