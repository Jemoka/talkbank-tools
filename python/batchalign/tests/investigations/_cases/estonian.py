"""Estonian probe cases (Tier B third batch, 2026-04-23).

Uralic (Finnic), agglutinative. All 5 cases 1-to-1 (observed
2026-04-23).
"""

from __future__ import annotations

from .._probe_types import Phenomenon, ProbeCase


CASES: tuple[ProbeCase, ...] = (
    ProbeCase("maja_alone", ("maja",), Phenomenon.CONTROL, expected_post_mwt_count=1),
    ProbeCase(
        "majas_alone",
        ("majas",),
        Phenomenon.CONTROL,
        expected_post_mwt_count=1,
    ),
    ProbeCase("kass_alone", ("kass",), Phenomenon.CONTROL, expected_post_mwt_count=1),
    ProbeCase(
        "ma_lähen_koju",
        ("ma", "lähen", "koju"),
        Phenomenon.CONTROL,
        expected_post_mwt_count=3,
    ),
    ProbeCase(
        "õpilane_alone",
        ("õpilane",),
        Phenomenon.CONTROL,
        expected_post_mwt_count=1,
    ),
)
