"""Indonesian probe cases (Tier B second batch, 2026-04-23).

Indonesian is Austronesian, broadly isolating. All 5 cases 1-to-1
under with-postprocessor. Hyphenated reduplication (``anak-anak``)
stays as one UD word.
"""

from __future__ import annotations

from .._probe_types import Phenomenon, ProbeCase


CASES: tuple[ProbeCase, ...] = (
    ProbeCase(
        "rumah_alone",
        ("rumah",),
        Phenomenon.CONTROL,
        expected_post_mwt_count=1,
    ),
    ProbeCase("anak_alone", ("anak",), Phenomenon.CONTROL, expected_post_mwt_count=1),
    ProbeCase(
        "anak_anak_alone",
        ("anak-anak",),
        Phenomenon.CONTROL,
        expected_post_mwt_count=1,
    ),
    ProbeCase(
        "dirumah_alone",
        ("dirumah",),
        Phenomenon.CONTROL,
        expected_post_mwt_count=1,
    ),
    ProbeCase(
        "saya_pergi_ke_rumah",
        ("saya", "pergi", "ke", "rumah"),
        Phenomenon.CONTROL,
        expected_post_mwt_count=4,
    ),
)
