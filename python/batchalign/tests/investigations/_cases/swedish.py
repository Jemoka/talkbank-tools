"""Swedish probe cases (Tier B second batch, 2026-04-23).

Swedish is Germanic. All 5 cases 1-to-1 under with-postprocessor
(definite suffixes, compounds, and plain forms stay as single
UD words).
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
    ProbeCase(
        "husvagn_alone",
        ("husvagn",),
        Phenomenon.CONTROL,
        expected_post_mwt_count=1,
    ),
    ProbeCase(
        "jag_går_hem",
        ("jag", "går", "hem"),
        Phenomenon.CONTROL,
        expected_post_mwt_count=3,
    ),
    ProbeCase(
        "barnen_alone",
        ("barnen",),
        Phenomenon.CONTROL,
        expected_post_mwt_count=1,
    ),
)
