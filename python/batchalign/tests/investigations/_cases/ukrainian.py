"""Ukrainian probe cases (Tier B third batch, 2026-04-23).

East Slavic. All 5 cases 1-to-1 (observed 2026-04-23).
"""

from __future__ import annotations

from .._probe_types import Phenomenon, ProbeCase


CASES: tuple[ProbeCase, ...] = (
    ProbeCase(
        "budynok_alone",
        ("будинок",),
        Phenomenon.CONTROL,
        expected_post_mwt_count=1,
    ),
    ProbeCase(
        "kishka_alone",
        ("кішка",),
        Phenomenon.CONTROL,
        expected_post_mwt_count=1,
    ),
    ProbeCase(
        "ya_idu_dodomu",
        ("я", "йду", "додому"),
        Phenomenon.CONTROL,
        expected_post_mwt_count=3,
    ),
    ProbeCase(
        "diakuyu_alone",
        ("дякую",),
        Phenomenon.CONTROL,
        expected_post_mwt_count=1,
    ),
    ProbeCase("vin_alone", ("він",), Phenomenon.CONTROL, expected_post_mwt_count=1),
)
