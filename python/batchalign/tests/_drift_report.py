"""Drift-sentinel reporting (Phase G of the test-cost revamp).

Probes (``@pytest.mark.mwt_probe`` and ``@pytest.mark.decision_probe``)
are monitors, not tests. They pin Stanza's current tokenization /
normalization behavior so a library upgrade that shifts behavior
surfaces as a diff. A probe "failure" warrants adjudication at
leisure — it should never block a push or a merge.

This module consumes the Phase B test-history SQLite (one row per
test invocation) and summarizes the most recent probe-run outcomes
into a markdown report. The runner script writes the report into
``docs/investigations/drift-*.md`` and opens an issue when any
drift is present — but never returns a non-zero exit code to the
caller.

Deliberately independent of pytest's own reporting machinery so a
crashed probe run (model failed to load, etc.) still produces a
report — the absence of expected rows in the DB is itself a
signal.
"""

from __future__ import annotations

import sqlite3
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class ProbeOutcome:
    """The most recent outcome of a single probe test."""

    test_id: str
    outcome: str  # pytest outcome literal: "passed" / "failed" / ...
    duration_s: float


@dataclass(frozen=True)
class DriftReport:
    """Aggregate view of the most-recent-per-probe outcomes.

    ``drift_outcomes`` holds the subset of probes whose latest
    outcome is not ``"passed"``. That's what the caller surfaces as
    issues — a non-empty list means drift needs review.
    """

    total_probes: int
    passes: int
    fails: int
    drift_outcomes: list[ProbeOutcome]


def build_drift_report(db_path: Path, *, since_ts: int) -> DriftReport:
    """Query the history DB for probe runs in ``[since_ts, now]``.

    Dedupes to the most recent outcome per ``test_id`` — a probe that
    failed earlier in the window but passed after a re-run is not
    current drift. Returns an empty report when the DB doesn't exist
    (first-ever run) or the window has no rows.
    """
    if not db_path.exists():
        return DriftReport(
            total_probes=0, passes=0, fails=0, drift_outcomes=[]
        )

    conn = sqlite3.connect(db_path)
    try:
        rows = conn.execute(
            # Self-join / GROUP BY trick to get the latest ts per test_id
            # within the window. SQLite doesn't have window functions
            # across all supported versions, so do it with a subquery.
            """
            SELECT r.test_id, r.outcome, r.duration_s
            FROM test_runs r
            INNER JOIN (
                SELECT test_id, MAX(ts) AS max_ts
                FROM test_runs
                WHERE ts >= ? AND framework = 'pytest'
                GROUP BY test_id
            ) latest
              ON r.test_id = latest.test_id AND r.ts = latest.max_ts
            WHERE r.framework = 'pytest' AND r.ts >= ?
            """,
            (since_ts, since_ts),
        ).fetchall()
    finally:
        conn.close()

    total = len(rows)
    passes = sum(1 for _, outcome, _ in rows if outcome == "passed")
    fails = total - passes
    drift = [
        ProbeOutcome(test_id=tid, outcome=oc, duration_s=dur)
        for tid, oc, dur in rows
        if oc != "passed"
    ]
    return DriftReport(
        total_probes=total,
        passes=passes,
        fails=fails,
        drift_outcomes=drift,
    )


def format_markdown(report: DriftReport, *, run_date: str) -> str:
    """Render ``report`` as a markdown report for
    ``docs/investigations/drift-<date>-stanza.md``.

    The first line is a one-line summary so a reader scanning the
    investigations directory sees the verdict without opening the
    file. The body lists drift outcomes with their test_id.
    """
    header = (
        f"# Stanza drift probe report — {run_date}\n\n"
        f"**Status:** {'Drift detected' if report.fails else 'Clean'}\n"
        f"**Summary:** {report.passes} / {report.total_probes} passing"
    )
    if report.fails:
        header += f", **{report.fails} drift**"
    header += "\n\n"

    if not report.fails:
        return header + "No drift detected — all probe expectations match "\
            "current Stanza behavior.\n"

    body = "## Drift outcomes\n\n"
    body += "Each entry below is a probe whose most recent outcome is "\
        "not `passed`. Adjudicate whether the Stanza behavior change is:\n"
    body += "\n"
    body += "* acceptable → update the probe expectation to the new baseline\n"
    body += "* regression → file an upstream issue and pin the affected "\
        "Stanza version\n"
    body += "* our mistake → fix BA3's postprocessor and re-run the probes\n"
    body += "\n"
    body += "| test_id | outcome | duration_s |\n"
    body += "|---------|---------|------------|\n"
    for o in report.drift_outcomes:
        body += f"| `{o.test_id}` | {o.outcome} | {o.duration_s:.3f} |\n"
    return header + body
