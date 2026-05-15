"""Turkish probe cases (Tier B second batch, 2026-04-23).

Turkish is agglutinative; orthographic words stay as 1 UD word
regardless of suffix depth. All 5 cases 1-to-1 under
with-postprocessor.
"""

from __future__ import annotations

from .._probe_types import Phenomenon, ProbeCase


CASES: tuple[ProbeCase, ...] = (
    ProbeCase("ev_alone", ("ev",), Phenomenon.CONTROL, expected_post_mwt_count=1),
    ProbeCase(
        "kitap_alone",
        ("kitap",),
        Phenomenon.CONTROL,
        expected_post_mwt_count=1,
    ),
    ProbeCase(
        "evlerinde_alone",
        ("evlerinde",),
        Phenomenon.CONTROL,
        expected_post_mwt_count=1,
    ),
    ProbeCase(
        "gidiyorum_alone",
        ("gidiyorum",),
        Phenomenon.CONTROL,
        expected_post_mwt_count=1,
    ),
    ProbeCase(
        "ben_okula_gidiyorum",
        ("ben", "okula", "gidiyorum"),
        Phenomenon.CONTROL,
        expected_post_mwt_count=3,
    ),
)
