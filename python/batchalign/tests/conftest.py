# Package-level conftest for batchalign tests.
#
# Shared fixtures live here.  For test doubles see doubles.py.
#
# SAFETY: OOM prevention hooks below enforce that golden/ML tests NEVER run
# with parallel xdist workers on machines with < 128 GB RAM. This prevents
# kernel-level OOM panics caused by multiple Stanza model instances (2-5 GB
# each) running concurrently.
#
# Incidents: 2026-03-19 (nextest parallel), 2026-03-23 (pytest -n 3 golden).

from __future__ import annotations

import os
import platform
import pytest


def _is_ci() -> bool:
    """Return True when running under any CI provider.

    Conservative: any of the common env vars signals CI. Interactive
    shells (ming, a developer laptop) have none of these set.
    """
    return any(os.environ.get(k) for k in ("CI", "GITHUB_ACTIONS", "BUILDKITE", "JENKINS_URL"))


_FAIL_FAST_CLI_FLAGS = ("-x", "--exitfirst", "--maxfail")
_FAILED_FIRST_CLI_FLAGS = ("--ff", "--failed-first", "--no-ff", "--no-failed-first")


def _user_passed_any(argv: list[str], flags: tuple[str, ...]) -> bool:
    """True if the user explicitly supplied any of the listed flags.

    pytest option defaults (e.g. ``maxfail=0``) are indistinguishable
    from "user passed ``--maxfail=0``" once argparse has run, so the
    only safe signal is the raw invocation argv.
    """
    return any(arg == flag or arg.startswith(flag + "=") for arg in argv for flag in flags)


def _apply_interactive_pytest_defaults(config: pytest.Config) -> None:
    """Interactive-only defaults: fail-fast + failed-first ordering.

    Goal: remove *waiting* from the interactive tight-loop. When a
    developer runs `uv run pytest` locally, abort on the first failure
    and put previously-failed tests first so the next invocation finds
    them immediately.

    CI keeps `--no-fail-fast` (pytest default) to preserve full failure
    reports — partial reports hide flake-vs-systemic patterns, and CI's
    signal latency is dominated by queue + setup, not test runtime.

    Called from ``pytest_configure`` below, not directly — keeps the
    ordering explicit (memory guard runs first, then this).
    """
    if _is_ci():
        return
    # Inspect raw argv because argparse collapses "user passed --maxfail=0"
    # and "default is 0" into the same config.option.maxfail value.
    argv = getattr(config, "invocation_params", None)
    argv_list = list(argv.args) if argv is not None else []
    if not _user_passed_any(argv_list, _FAIL_FAST_CLI_FLAGS):
        config.option.maxfail = 1
    if not _user_passed_any(argv_list, _FAILED_FIRST_CLI_FLAGS):
        config.option.failedfirst = True


def _get_system_ram_gb() -> int:
    """Return total system RAM in GB (macOS and Linux)."""
    try:
        if platform.system() == "Darwin":
            import subprocess
            result = subprocess.run(
                ["sysctl", "-n", "hw.memsize"],
                capture_output=True, text=True, timeout=5,
            )
            return int(result.stdout.strip()) // (1024 ** 3)
        else:
            with open("/proc/meminfo") as f:
                for line in f:
                    if line.startswith("MemTotal:"):
                        kb = int(line.split()[1])
                        return kb // (1024 * 1024)
    except Exception:
        pass
    return 0  # Unknown — be conservative


# ---------------------------------------------------------------------------
# Phase B2 — test-run history writer
# ---------------------------------------------------------------------------
#
# When BATCHALIGN_TEST_HISTORY_DB is set (and _OFF is not), the hooks
# below record every test's (test_id, outcome, duration) to a SQLite
# DB. Feed used by Phase E (historical-failure ordering) and Phase B3
# (weekly top-N slowest report). Default: silent / no-op.

_history_writer = None  # type: ignore[var-annotated]  # set in pytest_configure
_history_commit_sha: str | None = None
# xdist mirrors pytest_runtest_logreport to the controller AND each worker;
# recording in both places produces duplicate rows. _history_should_record
# resolves to True on exactly one side per test:
#   - No xdist: controller records.
#   - xdist active: only the worker that ran the test records.
_history_should_record: bool = False


def _resolve_commit_sha() -> str | None:
    """Return the current HEAD sha or None. Cached once per invocation."""
    import subprocess
    try:
        result = subprocess.run(
            ["git", "rev-parse", "--short=12", "HEAD"],
            capture_output=True,
            text=True,
            timeout=2,
            check=False,
        )
    except (FileNotFoundError, OSError):
        return None
    if result.returncode != 0:
        return None
    sha = result.stdout.strip()
    return sha or None


def _open_history_writer() -> object | None:
    """Create a HistoryWriter if BATCHALIGN_TEST_HISTORY_DB is set and _OFF isn't."""
    if os.environ.get("BATCHALIGN_TEST_HISTORY_OFF"):
        return None
    db_path = os.environ.get("BATCHALIGN_TEST_HISTORY_DB")
    if not db_path:
        return None
    from pathlib import Path
    from batchalign.tests._test_history import HistoryWriter
    return HistoryWriter(Path(db_path))


def pytest_configure(config: pytest.Config) -> None:
    """Block parallel golden test execution on machines with insufficient RAM.

    This hook fires before test collection. If the user requested golden tests
    (via -m golden) and xdist parallelism is active (-n > 0), either force
    sequential execution or abort with a clear error.

    Also installs interactive-only fail-fast + failed-first defaults;
    CI runs keep pytest's complete-report behavior.

    Also opens the history writer when BATCHALIGN_TEST_HISTORY_DB is
    set — see ``pytest_runtest_logreport`` below.
    """
    _apply_interactive_pytest_defaults(config)

    global _history_writer, _history_commit_sha, _history_should_record
    _history_writer = _open_history_writer()
    if _history_writer is not None:
        _history_commit_sha = _resolve_commit_sha()
        is_xdist_worker = hasattr(config, "workerinput")
        xdist_active = (
            getattr(config.option, "numprocesses", None) is not None
            and (getattr(config.option, "numprocesses", 0) or 0) > 0
        )
        # When xdist is active, only the worker writes (it actually ran the
        # test). Without xdist, the sole process writes.
        _history_should_record = is_xdist_worker or not xdist_active

    ram_gb = _get_system_ram_gb()

    # Detect if xdist is active and how many workers
    num_workers = getattr(config.option, "numprocesses", None)
    if num_workers is None:
        # xdist not installed or not active
        return

    # num_workers can be "auto" string or int
    if isinstance(num_workers, str):
        if num_workers == "auto":
            import os
            num_workers = os.cpu_count() or 1
        else:
            try:
                num_workers = int(num_workers)
            except ValueError:
                return

    if num_workers <= 0:
        # -n 0 means no parallelism — safe
        return

    # Check if golden tests are being INCLUDED (not excluded).
    # Default pytest.ini uses "not slow and not golden and not integration" which
    # EXCLUDES golden. We only need protection when golden is actively selected.
    markexpr = config.option.markexpr or ""
    # "golden" included but NOT preceded by "not " — means golden tests are selected
    has_golden = "golden" in markexpr and "not golden" not in markexpr

    if has_golden and ram_gb > 0:
        # Phase D: replace the 128 GB cliff with budgeted parallelism.
        # Each golden worker loads Stanza/Whisper/... (~12 GB peak RSS);
        # compute how many fit in 60% of the system RAM.
        from batchalign.tests._memory_budget import budgeted_jobs
        budget_n = budgeted_jobs("ml", total_ram_mb=ram_gb * 1024)
        budget_n = min(budget_n, num_workers)  # never INCREASE concurrency

        if budget_n < num_workers:
            import warnings
            config.option.numprocesses = budget_n
            config.option.dist = "no" if budget_n == 0 else config.option.dist
            if budget_n == 0:
                warnings.warn(
                    f"\n  OOM PROTECTION: forced -n 0 for golden tests on this "
                    f"{ram_gb} GB host (budget: 0 ml workers).\n"
                    f"  To run golden tests in parallel, use a host with more RAM.\n",
                    stacklevel=1,
                )
            else:
                warnings.warn(
                    f"\n  OOM PROTECTION: clipped -n {num_workers} → -n {budget_n} "
                    f"for golden tests on this {ram_gb} GB host (ml peak-RSS budget).\n",
                    stacklevel=1,
                )


_HISTORY_PRIORITY_WINDOW_SECONDS = 7 * 24 * 3600  # 7 days


def _apply_history_priority_ordering(
    config: pytest.Config, items: list[pytest.Item]
) -> None:
    """Reorder ``items`` so likely-failing tests run first.

    Reads the last 7 days of test history from the SQLite DB (the
    same one Phase B's writer feeds). Only runs when:
      - BATCHALIGN_TEST_HISTORY_DB is set (there's a DB to read).
      - Not in CI (reproducibility > speed; the same PR should run
        tests in the same order on every CI invocation).

    Silently no-ops on any unexpected condition — ordering is a
    free-lunch optimization, not correctness, and should never break
    a run.
    """
    if _is_ci():
        return
    db_path_str = os.environ.get("BATCHALIGN_TEST_HISTORY_DB")
    if not db_path_str:
        return
    from pathlib import Path
    db_path = Path(db_path_str)
    if not db_path.exists():
        return
    try:
        import time
        from batchalign.tests._history_priority import load_stats, order_by_priority
        since_ts = int(time.time()) - _HISTORY_PRIORITY_WINDOW_SECONDS
        stats = load_stats(db_path, since_ts=since_ts)
    except Exception:
        return  # Any read failure is non-fatal — fall back to collection order.
    if not stats:
        return

    # Map nodeid → item, order the nodeids, rebuild items in that order.
    by_nodeid = {item.nodeid: item for item in items}
    ordered_ids = order_by_priority(list(by_nodeid), stats)
    items[:] = [by_nodeid[nid] for nid in ordered_ids]


def pytest_collection_modifyitems(
    config: pytest.Config, items: list[pytest.Item]
) -> None:
    """Second safety net: fire if golden tests are collected with
    parallelism that exceeds the memory budget.

    The configure-time guard above already clips ``numprocesses`` to
    the budget. This fixture catches the edge case where
    ``numprocesses`` is set AFTER configure (e.g. via a plugin) or
    where collection picks up more golden tests than the configure
    heuristic expected.

    Also applies Phase E historical-failure ordering: reorder
    ``items`` so likely-failing tests run first. See
    ``_apply_history_priority_ordering`` below.
    """
    _apply_history_priority_ordering(config, items)

    num_workers = getattr(config.option, "numprocesses", None)

    # Normalize num_workers to int
    if num_workers is not None and not isinstance(num_workers, int):
        try:
            num_workers = int(num_workers)
        except (ValueError, TypeError):
            import os
            num_workers = os.cpu_count() or 1

    if num_workers is None or num_workers <= 0:
        return

    ram_gb = _get_system_ram_gb()
    if ram_gb <= 0:
        return  # Unknown RAM — trust the configure-time guard's serialization.

    from batchalign.tests._memory_budget import budgeted_jobs
    budget_n = budgeted_jobs("ml", total_ram_mb=ram_gb * 1024)
    if num_workers <= budget_n:
        return

    golden_tests = [item for item in items if item.get_closest_marker("golden")]
    if golden_tests:
        pytest.exit(
            f"REFUSED: {len(golden_tests)} golden test(s) collected with "
            f"-n {num_workers} on a {ram_gb} GB host (budget: {budget_n}). "
            f"Each ML worker needs ~12 GB peak RSS. "
            f"Re-run with -n {budget_n} or fewer.",
            returncode=1,
        )


@pytest.fixture(autouse=True)
def _guard_golden_oom(request: pytest.FixtureRequest) -> None:
    """Per-test OOM guard — belt-and-suspenders for the collect-time
    budget check.

    Fires only when we're in an xdist worker on a host so small the
    budget says "zero ml workers allowed." In that case something
    bypassed the configure + collection guards and the test would
    crash if it tried to load a model — fail early with a clear
    message instead.
    """
    marker = request.node.get_closest_marker("golden")
    if marker is None:
        return

    # Not in an xdist worker — the sole process is always OK for a
    # single golden test (enough RAM to host one model at a time).
    worker_id = getattr(request.config, "workerinput", {}).get("workerid", None)
    if worker_id is None:
        return

    ram_gb = _get_system_ram_gb()
    if ram_gb <= 0:
        return

    from batchalign.tests._memory_budget import budgeted_jobs
    budget_n = budgeted_jobs("ml", total_ram_mb=ram_gb * 1024)
    if budget_n > 0:
        return  # budget allows this many parallel workers; we're fine.

    pytest.fail(
        f"OOM PROTECTION: golden test '{request.node.name}' running in "
        f"xdist worker {worker_id} on a {ram_gb} GB host where the ml "
        f"budget is 0 workers. Use -n 0 to force serialized execution."
    )


def pytest_runtest_logreport(report: pytest.TestReport) -> None:
    """Record the outcome + duration of every test's ``call`` phase.

    Setup/teardown durations are excluded — they're noisy and not what
    the historical-failure-ordering predictor needs. If a test fails in
    setup, pytest emits a ``call`` phase with ``error`` outcome, so we
    still catch errors there.
    """
    if _history_writer is None or report.when != "call":
        return
    if not _history_should_record:
        return
    _history_writer.record(  # type: ignore[attr-defined]
        report.nodeid,
        report.outcome,
        report.duration,
        commit_sha=_history_commit_sha,
        framework="pytest",
    )


def pytest_unconfigure(config: pytest.Config) -> None:
    """Close the history writer on session end. Idempotent."""
    global _history_writer
    if _history_writer is not None:
        _history_writer.close()  # type: ignore[attr-defined]
        _history_writer = None
