"""Memory-budget parallelism for pytest xdist.

Phase D of the test-cost revamp. Mirrors the constants in
``scripts/choose-test-concurrency.sh`` (used for ad-hoc nextest
invocations) so pytest and cargo-nextest share one mental model of
"how many of these can I run at once."

The previous policy was a binary cliff: if system RAM was below 128
GB, the OOM guard forced ``-n 0`` for any run containing golden
tests. Graduated parallelism replaces that cliff with budgeted
arithmetic:

    usable_mb = total_mb × (1 − reserve_fraction)
    jobs      = usable_mb // peak_rss_mb(profile)

The hard refusal behavior is preserved for hosts so small the
computed jobs count drops to zero — the caller forces serial
execution and warns.

Profile peak-RSS values are empirical estimates, not contractual
budgets. Update them when Phase B measurements show drift.
"""

from __future__ import annotations

from typing import Final

# Per-profile peak resident-set estimates. Keep in sync with
# ``scripts/choose-test-concurrency.sh``; the shell script copy
# serves the nextest path, this module serves the pytest path.
_PEAK_RSS_MB: Final[dict[str, int]] = {
    "default": 1024,
    "python": 1024,
    "stress": 4096,
    "gpu": 6144,
    "ml": 12288,
}

# Default slice of system RAM reserved for everything that isn't the
# test process: OS, editor, browsers, databases, etc. 40% is an
# empirically safe reserve on macOS developer boxes.
DEFAULT_RESERVE_FRACTION: Final[float] = 0.4


class BudgetError(ValueError):
    """Raised when a profile name is unknown or an arg is out of range."""


def peak_rss_mb(profile: str) -> int:
    """Return the peak-RSS estimate for ``profile`` in megabytes."""
    try:
        return _PEAK_RSS_MB[profile]
    except KeyError as exc:
        raise BudgetError(
            f"unknown profile {profile!r}; "
            f"known: {sorted(_PEAK_RSS_MB)}"
        ) from exc


def budgeted_jobs(
    profile: str,
    *,
    total_ram_mb: int,
    reserve_fraction: float = DEFAULT_RESERVE_FRACTION,
) -> int:
    """Return a memory-bound safe parallel-jobs count.

    Returns 0 when the host is too small to safely run even one
    worker of ``profile``. Callers that want the CPU ceiling
    (``os.cpu_count``) should clip this value themselves; we don't
    import ``os`` here to keep this pure and unit-testable.

    Raises :class:`BudgetError` on an unknown profile or an out-of-
    range ``reserve_fraction`` (valid range: 0.0 ≤ r < 1.0).
    """
    if not 0.0 <= reserve_fraction < 1.0:
        raise BudgetError(
            f"reserve_fraction must be in [0.0, 1.0); got {reserve_fraction}"
        )
    if total_ram_mb <= 0:
        # Unknown RAM → refuse parallel execution. Caller serializes.
        return 0
    peak = peak_rss_mb(profile)
    usable_mb = int(total_ram_mb * (1.0 - reserve_fraction))
    return usable_mb // peak
