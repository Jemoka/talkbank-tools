"""Finnish probe cases (Tier B third batch, 2026-04-23).

Uralic, highly agglutinative. All 5 cases 1-to-1 (observed
2026-04-23). Stacked case-number-possessive suffixes
(``taloissani`` = "in my houses") stay as one UD word.
"""

from __future__ import annotations

from .._probe_types import Phenomenon, ProbeCase


CASES: tuple[ProbeCase, ...] = (
    ProbeCase("talo_alone", ("talo",), Phenomenon.CONTROL, expected_post_mwt_count=1),
    ProbeCase(
        "talossa_alone",
        ("talossa",),
        Phenomenon.CONTROL,
        expected_post_mwt_count=1,
    ),
    ProbeCase(
        "taloissani_alone",
        ("taloissani",),
        Phenomenon.CONTROL,
        expected_post_mwt_count=1,
    ),
    ProbeCase(
        "mina_menen_kotiin",
        ("minä", "menen", "kotiin"),
        Phenomenon.CONTROL,
        expected_post_mwt_count=3,
    ),
    ProbeCase(
        "kissa_alone",
        ("kissa",),
        Phenomenon.CONTROL,
        expected_post_mwt_count=1,
    ),
)
