"""Czech probe cases (Tier B third batch, 2026-04-23).

Slavic, rich morphology, no articles. All 5 cases 1-to-1 under
with-postprocessor (observed 2026-04-23 first golden run).
"""

from __future__ import annotations

from .._probe_types import Phenomenon, ProbeCase


CASES: tuple[ProbeCase, ...] = (
    ProbeCase("dum_alone", ("dům",), Phenomenon.CONTROL, expected_post_mwt_count=1),
    ProbeCase(
        "kocka_alone",
        ("kočka",),
        Phenomenon.CONTROL,
        expected_post_mwt_count=1,
    ),
    ProbeCase(
        "jdu_domu_plain",
        ("jdu", "domů"),
        Phenomenon.CONTROL,
        expected_post_mwt_count=2,
    ),
    ProbeCase(
        "delate_alone",
        ("děláte",),
        Phenomenon.CONTROL,
        expected_post_mwt_count=1,
    ),
    ProbeCase(
        "muj_kamarad_je_doma",
        ("můj", "kamarád", "je", "doma"),
        Phenomenon.CONTROL,
        expected_post_mwt_count=4,
    ),
)
