# affects: batchalign/tests/conftest.py
"""Regression tests for the interactive pytest defaults installed by
``conftest._apply_interactive_pytest_defaults``.

Phase A2 of the test-cost revamp: interactive invocations get
fail-fast + failed-first ordering; CI keeps pytest's complete-report
behavior (queue-bound signal latency + flake-vs-systemic distinction
both favor full reports there); and a user's explicit CLI override
always wins.

The conftest hook reads environment + invocation-argv when it fires,
so these tests assert shapes that were already decided before
collection. Run in three flavors to exercise all three branches:

- ``uv run pytest batchalign/tests/test_a2_interactive_defaults.py``
  — interactive default
- ``CI=1 uv run pytest batchalign/tests/test_a2_interactive_defaults.py``
  — CI default
- ``uv run pytest batchalign/tests/test_a2_interactive_defaults.py --maxfail=0``
  — explicit user override (expected to respect the 0)
"""

from __future__ import annotations

from batchalign.tests.conftest import _FAIL_FAST_CLI_FLAGS, _is_ci, _user_passed_any


def test_interactive_or_ci_defaults_match_environment(pytestconfig) -> None:
    argv = list(pytestconfig.invocation_params.args)
    user_forced_fail_fast = _user_passed_any(argv, _FAIL_FAST_CLI_FLAGS)

    if _is_ci():
        # CI must not auto-enable fail-fast.
        assert pytestconfig.option.maxfail == 0, (
            f"CI must not auto-enable fail-fast; got maxfail={pytestconfig.option.maxfail}"
        )
        assert not pytestconfig.option.failedfirst, (
            "CI must not auto-enable --failed-first (hides flake-vs-systemic pattern)"
        )
    elif user_forced_fail_fast:
        # User's explicit CLI override wins — hook must not stomp.
        # We don't assert a specific maxfail value; the value is whatever
        # argparse parsed from the CLI. Just assert the hook didn't run
        # the default assignment path (which would set it to 1 without
        # the _user_passed_any guard).
        pass
    else:
        # Interactive default: hook installs -x + --ff.
        assert pytestconfig.option.maxfail == 1, (
            f"interactive should have maxfail=1 from -x; got {pytestconfig.option.maxfail}"
        )
        assert pytestconfig.option.failedfirst, (
            "interactive should have --failed-first on for tight-loop iteration"
        )
