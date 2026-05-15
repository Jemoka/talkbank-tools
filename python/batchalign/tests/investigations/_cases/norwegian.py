"""Norwegian Bokmål probe cases (Tier B third batch, 2026-04-23).

North Germanic. All 5 cases 1-to-1 (observed 2026-04-23).
"""

from __future__ import annotations

from .._probe_types import Phenomenon, ProbeCase


CASES: tuple[ProbeCase, ...] = (
    ProbeCase("hus_alone", ("hus",), Phenomenon.CONTROL, expected_post_mwt_count=1),
    ProbeCase(
        "huset_alone",
        ("huset",),
        Phenomenon.CONTROL,
        expected_post_mwt_count=1,
    ),
    ProbeCase("katt_alone", ("katt",), Phenomenon.CONTROL, expected_post_mwt_count=1),
    ProbeCase(
        "jeg_går_hjem",
        ("jeg", "går", "hjem"),
        Phenomenon.CONTROL,
        expected_post_mwt_count=3,
    ),
    ProbeCase(
        "barnehage_alone",
        ("barnehage",),
        Phenomenon.CONTROL,
        expected_post_mwt_count=1,
    ),
)
