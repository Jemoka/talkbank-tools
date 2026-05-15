# affects: batchalign/tests/_drift_report.py
"""Unit tests for the drift-sentinel report generator (Phase G of
the test-cost revamp).

Probes (``mwt_probe``, ``decision_probe``) are monitors, not tests.
A probe "failure" indicates Stanza behavior drifted from the pinned
expectation — worth surfacing as an issue to adjudicate, never
worth blocking a push. The report generator turns probe-run
outcomes into a human-readable markdown file that sits in
``docs/investigations/``.

Tests use synthetic history-DB rows (reusing the Phase B SQLite
schema) so the generator is testable without running real Stanza.
"""

from __future__ import annotations

import sqlite3
import time
from pathlib import Path

from batchalign.tests._drift_report import (
    DriftReport,
    ProbeOutcome,
    build_drift_report,
    format_markdown,
)
from batchalign.tests._test_history import SCHEMA_DDL


def _seed_probe_runs(
    db: Path, rows: list[tuple[int, str, str, float]]
) -> None:
    """Seed the Phase B history DB with probe runs.

    rows: ``(ts, test_id, outcome, duration_s)``.
    """
    db.parent.mkdir(parents=True, exist_ok=True)
    conn = sqlite3.connect(db)
    try:
        conn.executescript(SCHEMA_DDL)
        conn.executemany(
            "INSERT INTO test_runs "
            "(ts, test_id, outcome, duration_s, commit_sha, framework) "
            "VALUES (?, ?, ?, ?, NULL, 'pytest')",
            rows,
        )
        conn.commit()
    finally:
        conn.close()


# ---------- build_drift_report -----------------------------------------


def test_single_run_all_passing(tmp_path: Path) -> None:
    db = tmp_path / "probes.sqlite"
    now = int(time.time())
    _seed_probe_runs(
        db,
        [
            (now, "tests/test_mwt_probe.py::test_case_a", "passed", 0.3),
            (now, "tests/test_mwt_probe.py::test_case_b", "passed", 0.4),
        ],
    )

    report = build_drift_report(db, since_ts=now - 60)
    assert report.total_probes == 2
    assert report.passes == 2
    assert report.fails == 0
    assert report.drift_outcomes == []


def test_single_run_with_failures(tmp_path: Path) -> None:
    db = tmp_path / "probes.sqlite"
    now = int(time.time())
    _seed_probe_runs(
        db,
        [
            (now, "tests/test_mwt_probe.py::test_case_a", "passed", 0.3),
            (now, "tests/test_mwt_probe.py::test_case_b", "failed", 0.5),
            (now, "tests/test_decision_probe.py::test_c", "failed", 0.2),
        ],
    )

    report = build_drift_report(db, since_ts=now - 60)
    assert report.total_probes == 3
    assert report.passes == 1
    assert report.fails == 2
    assert {o.test_id for o in report.drift_outcomes} == {
        "tests/test_mwt_probe.py::test_case_b",
        "tests/test_decision_probe.py::test_c",
    }


def test_only_most_recent_run_per_probe_counts(tmp_path: Path) -> None:
    """A probe that failed yesterday but passes today should not count
    as current drift. The report windows by ``since_ts`` but also
    dedupes to the most-recent outcome per probe within the window."""
    db = tmp_path / "probes.sqlite"
    now = int(time.time())
    _seed_probe_runs(
        db,
        [
            (now - 3600, "p::t", "failed", 0.3),  # earlier
            (now, "p::t", "passed", 0.3),          # latest — dominates
        ],
    )
    report = build_drift_report(db, since_ts=now - 7200)
    assert report.total_probes == 1
    assert report.passes == 1
    assert report.fails == 0


def test_empty_window_returns_empty_report(tmp_path: Path) -> None:
    db = tmp_path / "probes.sqlite"
    _seed_probe_runs(db, [])
    report = build_drift_report(db, since_ts=0)
    assert report.total_probes == 0
    assert report.passes == 0
    assert report.fails == 0
    assert report.drift_outcomes == []


def test_missing_db_returns_empty_report(tmp_path: Path) -> None:
    missing = tmp_path / "not-here.sqlite"
    report = build_drift_report(missing, since_ts=0)
    assert report.total_probes == 0
    assert report.drift_outcomes == []


# ---------- format_markdown --------------------------------------------


def test_format_no_drift() -> None:
    report = DriftReport(total_probes=100, passes=100, fails=0, drift_outcomes=[])
    md = format_markdown(report, run_date="2026-04-23")
    assert "100 / 100 passing" in md
    assert "No drift detected" in md


def test_format_with_drift() -> None:
    outcomes = [
        ProbeOutcome(
            test_id="batchalign/tests/investigations/x.py::test_a",
            outcome="failed",
            duration_s=0.3,
        ),
        ProbeOutcome(
            test_id="batchalign/tests/investigations/y.py::test_b",
            outcome="failed",
            duration_s=1.2,
        ),
    ]
    report = DriftReport(total_probes=50, passes=48, fails=2, drift_outcomes=outcomes)
    md = format_markdown(report, run_date="2026-04-23")
    assert "48 / 50 passing" in md
    assert "2 drift" in md
    assert "test_a" in md
    assert "test_b" in md


def test_format_reports_date() -> None:
    report = DriftReport(total_probes=0, passes=0, fails=0, drift_outcomes=[])
    md = format_markdown(report, run_date="2026-05-15")
    assert "2026-05-15" in md
