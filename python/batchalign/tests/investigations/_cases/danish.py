"""Danish probe cases (Tier B third batch, 2026-04-23).

North Germanic. All 5 cases 1-to-1 (observed 2026-04-23).
"""

from __future__ import annotations

from .._probe_types import Phenomenon, ProbeCase


CASES: tuple[ProbeCase, ...] = (
    ProbeCase.strict_alone("hus"),
    ProbeCase.strict_alone("huset"),
    ProbeCase.strict_alone("kat"),
    ProbeCase(
        "jeg_går_hjem",
        ("jeg", "går", "hjem"),
        Phenomenon.CONTROL,
        expected_post_mwt_count=3,
    ),
    ProbeCase(
        "børnehave_alone",
        ("børnehave",),
        Phenomenon.CONTROL,
        expected_post_mwt_count=1,
    ),
)
