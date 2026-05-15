# affects: batchalign/tests/_test_history.py
"""Unit tests for the per-test history writer (Phase B2 of the
test-cost revamp).

The writer is a thin SQLite wrapper: one row per test invocation,
keyed by ``(ts, test_id)``. A pytest hook in ``conftest.py`` feeds
it from ``pytest_runtest_logreport``; nextest can feed it from
post-run JSON parsing in a separate pass.

Tests cover:
  - Schema creation on first open (idempotent).
  - Round-trip of a single record.
  - Outcome, duration, commit_sha, framework fields persist.
  - Parent directory is created if missing.
  - Writer is resilient to being closed twice.
"""

from __future__ import annotations

import sqlite3
from pathlib import Path

import pytest

from batchalign.tests._test_history import HistoryWriter


def test_writer_creates_schema(tmp_path: Path) -> None:
    db = tmp_path / "hist.sqlite"
    writer = HistoryWriter(db)
    writer.close()

    conn = sqlite3.connect(db)
    try:
        tables = {row[0] for row in conn.execute(
            "SELECT name FROM sqlite_master WHERE type='table'"
        )}
    finally:
        conn.close()

    assert "test_runs" in tables


def test_record_round_trips(tmp_path: Path) -> None:
    db = tmp_path / "hist.sqlite"
    writer = HistoryWriter(db)
    writer.record(
        "mod::test_a",
        "passed",
        0.123,
        commit_sha="abc123",
        framework="pytest",
    )
    writer.close()

    conn = sqlite3.connect(db)
    try:
        rows = conn.execute(
            "SELECT test_id, outcome, duration_s, commit_sha, framework "
            "FROM test_runs"
        ).fetchall()
    finally:
        conn.close()

    assert rows == [("mod::test_a", "passed", 0.123, "abc123", "pytest")]


def test_multiple_records_preserve_order(tmp_path: Path) -> None:
    db = tmp_path / "hist.sqlite"
    writer = HistoryWriter(db)
    writer.record("t::one", "passed", 0.1)
    writer.record("t::two", "failed", 0.5)
    writer.record("t::three", "skipped", 0.01)
    writer.close()

    conn = sqlite3.connect(db)
    try:
        rows = conn.execute(
            "SELECT test_id, outcome FROM test_runs ORDER BY ts, rowid"
        ).fetchall()
    finally:
        conn.close()

    assert rows == [
        ("t::one", "passed"),
        ("t::two", "failed"),
        ("t::three", "skipped"),
    ]


def test_parent_directory_created(tmp_path: Path) -> None:
    nested = tmp_path / "a" / "b" / "c"
    db = nested / "hist.sqlite"
    assert not nested.exists()

    writer = HistoryWriter(db)
    writer.close()

    assert db.exists()


def test_commit_sha_optional(tmp_path: Path) -> None:
    db = tmp_path / "hist.sqlite"
    writer = HistoryWriter(db)
    writer.record("t::x", "passed", 0.1)
    writer.close()

    conn = sqlite3.connect(db)
    try:
        row = conn.execute(
            "SELECT commit_sha FROM test_runs"
        ).fetchone()
    finally:
        conn.close()

    assert row == (None,)


def test_timestamp_is_recorded(tmp_path: Path) -> None:
    import time

    db = tmp_path / "hist.sqlite"
    before = int(time.time())
    writer = HistoryWriter(db)
    writer.record("t::x", "passed", 0.1)
    writer.close()
    after = int(time.time())

    conn = sqlite3.connect(db)
    try:
        row = conn.execute("SELECT ts FROM test_runs").fetchone()
    finally:
        conn.close()

    assert row is not None
    assert before <= row[0] <= after


def test_close_is_idempotent(tmp_path: Path) -> None:
    db = tmp_path / "hist.sqlite"
    writer = HistoryWriter(db)
    writer.close()
    writer.close()  # Must not raise.


def test_record_after_close_raises(tmp_path: Path) -> None:
    db = tmp_path / "hist.sqlite"
    writer = HistoryWriter(db)
    writer.close()

    with pytest.raises(sqlite3.ProgrammingError):
        writer.record("t::x", "passed", 0.1)
