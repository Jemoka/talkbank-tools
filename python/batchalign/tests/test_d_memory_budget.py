# affects: batchalign/tests/_memory_budget.py
# affects: batchalign/tests/conftest.py
"""Unit tests for the memory-budget parallelism helper (Phase D of
the test-cost revamp).

The helper computes a safe xdist worker count by dividing the
usable RAM (total × (1 − reserve_fraction)) by the profile's peak
per-worker RSS. Replaces the old binary 128 GB cliff in conftest
with graduated parallelism:

  * ming (256 GB, ml=12 GB peak) → 21 workers capped at CPU count.
  * 64 GB laptop, ml=12 GB      → 3 workers.
  * 32 GB laptop, ml=12 GB      → 1 worker (serialized).
  * 16 GB laptop, ml=12 GB      → 0 workers (refuse, too little RAM).

Tests use injected ``total_ram_mb`` to make the math deterministic;
the production API reads real system memory via
``_get_system_ram_gb`` in conftest.
"""

from __future__ import annotations

import pytest

from batchalign.tests._memory_budget import (
    BudgetError,
    budgeted_jobs,
    peak_rss_mb,
)


def test_peak_rss_known_profiles() -> None:
    assert peak_rss_mb("default") == 1024
    assert peak_rss_mb("python") == 1024
    assert peak_rss_mb("stress") == 4096
    assert peak_rss_mb("gpu") == 6144
    assert peak_rss_mb("ml") == 12288


def test_peak_rss_unknown_profile_raises() -> None:
    with pytest.raises(BudgetError, match="unknown profile"):
        peak_rss_mb("bogus")


def test_budgeted_jobs_ml_on_ming() -> None:
    # 256 GB * 0.6 = 153 600 MB; / 12 288 MB = 12.5 → 12.
    # CPU count not capped here (budgeted_jobs returns the memory-bound
    # count; caller clips to CPU).
    assert budgeted_jobs("ml", total_ram_mb=256 * 1024) == 12


def test_budgeted_jobs_ml_on_64gb_laptop() -> None:
    # 64 GB * 0.6 = 38 400 MB; / 12 288 MB = 3.125 → 3.
    assert budgeted_jobs("ml", total_ram_mb=64 * 1024) == 3


def test_budgeted_jobs_ml_on_32gb_laptop() -> None:
    # 32 GB * 0.6 = 19 200 MB; / 12 288 MB = 1.56 → 1.
    assert budgeted_jobs("ml", total_ram_mb=32 * 1024) == 1


def test_budgeted_jobs_ml_on_16gb_too_small() -> None:
    # 16 GB * 0.6 = 9 830 MB; / 12 288 MB = 0.8 → 0.
    # Zero means "refuse to run parallel ML tests on this host".
    assert budgeted_jobs("ml", total_ram_mb=16 * 1024) == 0


def test_budgeted_jobs_default_on_small_host() -> None:
    # default profile is 1 GB per worker, so even 16 GB * 0.6 = 9 G →
    # 9 workers.
    assert budgeted_jobs("default", total_ram_mb=16 * 1024) == 9


def test_budgeted_jobs_custom_reserve() -> None:
    # 100 GB * 0.8 reserve fraction = 20 GB usable = 20 480 MB.
    # 20 480 / 12 288 = 1.66 → 1.
    assert budgeted_jobs("ml", total_ram_mb=100 * 1024, reserve_fraction=0.8) == 1


def test_budgeted_jobs_zero_ram_returns_zero() -> None:
    # Unknown RAM (detection failed) → refuse parallel execution.
    assert budgeted_jobs("ml", total_ram_mb=0) == 0


def test_budgeted_jobs_rejects_bad_reserve_fraction() -> None:
    with pytest.raises(BudgetError, match="reserve_fraction"):
        budgeted_jobs("ml", total_ram_mb=64 * 1024, reserve_fraction=1.5)
    with pytest.raises(BudgetError, match="reserve_fraction"):
        budgeted_jobs("ml", total_ram_mb=64 * 1024, reserve_fraction=-0.1)


def test_budgeted_jobs_unknown_profile_raises() -> None:
    with pytest.raises(BudgetError, match="unknown profile"):
        budgeted_jobs("bogus", total_ram_mb=64 * 1024)


# ---------- cross-language drift check ----------------------------------


def test_python_and_bash_peak_rss_tables_agree() -> None:
    """The Bash copy at scripts/choose-test-concurrency.sh must match
    this module's _PEAK_RSS_MB table.

    The two files share no runtime; the only guard against drift is
    this regression. If this fails, update one side to match the
    other — whichever side was intended — and re-run.
    """
    import re
    from pathlib import Path

    script = (
        Path(__file__).resolve().parent.parent.parent
        / "scripts"
        / "choose-test-concurrency.sh"
    )
    content = script.read_text()

    # case bodies look like:  default|python) peak_mb=1024 ;;
    case_re = re.compile(
        r"^\s*([A-Za-z0-9|]+)\)\s+peak_mb=(\d+)\s*;;",
        re.MULTILINE,
    )
    bash_table: dict[str, int] = {}
    for match in case_re.finditer(content):
        names = match.group(1).split("|")
        value = int(match.group(2))
        for name in names:
            bash_table[name] = value

    from batchalign.tests._memory_budget import _PEAK_RSS_MB
    # Every profile present in both tables must match.
    shared = set(bash_table) & set(_PEAK_RSS_MB)
    assert shared, "bash parser found no profiles — case-regex broken?"
    for name in shared:
        assert bash_table[name] == _PEAK_RSS_MB[name], (
            f"drift on profile {name!r}: "
            f"bash={bash_table[name]} vs python={_PEAK_RSS_MB[name]}"
        )
