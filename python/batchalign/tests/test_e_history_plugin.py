# affects: batchalign/tests/conftest.py
# affects: batchalign/tests/_history_priority.py
"""Integration test for the Phase E ordering hook.

Seeds a history DB with two tests — one flaky, one clean — runs
pytest in a subprocess with both collected, and checks that the
flaky test runs first (fail-fast + history ordering = real failure
surfaces in seconds).

The hook only runs when BATCHALIGN_TEST_HISTORY_DB is set and we're
not in CI. Subprocess scrubs CI env vars so the hook fires.
"""

from __future__ import annotations

import sqlite3
import textwrap
import time
from pathlib import Path

from batchalign.tests._subprocess_pytest import (
    run_pytest_subprocess,
    subprocess_test_filename,
)
from batchalign.tests._test_history import SCHEMA_DDL

_TWO_TESTS_BODY = textwrap.dedent(
    """
    def test_z_bad():
        assert True

    def test_a_good():
        assert True
    """
)


def _seed_history(db: Path, entries: list[tuple[str, str]]) -> None:
    """Write ``(test_id, outcome)`` pairs at varying recent timestamps."""
    db.parent.mkdir(parents=True, exist_ok=True)
    conn = sqlite3.connect(db)
    try:
        conn.executescript(SCHEMA_DDL)
        now = int(time.time())
        # Put all rows within the last day — well inside the 7-day
        # ordering window.
        conn.executemany(
            "INSERT INTO test_runs (ts, test_id, outcome, duration_s, framework) "
            "VALUES (?, ?, ?, ?, 'pytest')",
            [
                (now - 60 * i, test_id, outcome, 0.1)
                for i, (test_id, outcome) in enumerate(entries)
            ],
        )
        conn.commit()
    finally:
        conn.close()


def test_flaky_test_runs_before_clean_test(tmp_path: Path) -> None:
    """Given a history where `test_z_bad` failed recently and
    `test_a_good` never did, the conftest ordering hook should put
    `test_z_bad` first — despite alphabetical order favoring the
    good one.
    """
    slug = "e_ordering_subprocess"
    subprocess_filename = subprocess_test_filename(tmp_path, slug)

    db = tmp_path / "history.sqlite"
    # Seed: test_z_bad failed 3 of 5 times; test_a_good always passed.
    nodeid_prefix = "batchalign/tests/" + subprocess_filename
    _seed_history(
        db,
        [(f"{nodeid_prefix}::test_z_bad", oc) for oc in
            ("failed", "failed", "failed", "passed", "passed")]
        + [(f"{nodeid_prefix}::test_a_good", "passed") for _ in range(5)],
    )

    result = run_pytest_subprocess(
        tmp_path,
        {"BATCHALIGN_TEST_HISTORY_DB": str(db)},
        dummy_body=_TWO_TESTS_BODY,
        slug=slug,
        extra_args=("-v",),
    )

    assert result.returncode == 0, result.stdout + result.stderr
    bad_pos = result.stdout.find("test_z_bad")
    good_pos = result.stdout.find("test_a_good")
    assert bad_pos != -1 and good_pos != -1, result.stdout
    assert bad_pos < good_pos, (
        "history ordering should put the flaky test_z_bad BEFORE test_a_good "
        "despite alphabetical order favoring the good one — got:\n" + result.stdout
    )


def test_ci_env_disables_ordering(tmp_path: Path) -> None:
    """Under CI we preserve collection / source order. Seed a DB that
    would otherwise put test_z_bad first; run with CI=1; confirm the
    source order (test_a_good defined first, test_z_bad defined
    second) is preserved.
    """
    slug = "e_ordering_ci_subprocess"
    subprocess_filename = subprocess_test_filename(tmp_path, slug)
    # Body where test_a_good comes first in source, so source order
    # (the ordering hook won't run) sorts it before test_z_bad.
    ci_body = textwrap.dedent(
        """
        def test_a_good():
            assert True

        def test_z_bad():
            assert True
        """
    )

    db = tmp_path / "history.sqlite"
    nodeid_bad = f"batchalign/tests/{subprocess_filename}::test_z_bad"
    _seed_history(db, [(nodeid_bad, "failed") for _ in range(5)])

    result = run_pytest_subprocess(
        tmp_path,
        {"CI": "1", "BATCHALIGN_TEST_HISTORY_DB": str(db)},
        dummy_body=ci_body,
        slug=slug,
        keep_ci_env=True,  # don't strip our own CI=1
        extra_args=("-v",),
    )

    assert result.returncode == 0, result.stdout + result.stderr
    bad_pos = result.stdout.find("test_z_bad")
    good_pos = result.stdout.find("test_a_good")
    assert bad_pos != -1 and good_pos != -1, result.stdout
    # Collection order preserved: test_a_good appears BEFORE test_z_bad.
    assert good_pos < bad_pos, (
        "CI runs must preserve collection order for reproducibility:\n"
        + result.stdout
    )
