"""Polish probe cases (Tier B second batch, 2026-04-23).

Polish is Slavic. Morphology is rich but tokenization is 1-to-1.
Observation (2026-04-23 first golden run): all 5 cases produce
1-to-1 UD-word counts under the with-postprocessor pipeline.
"""

from __future__ import annotations

from .._probe_types import Phenomenon, ProbeCase


CASES: tuple[ProbeCase, ...] = (
    ProbeCase.strict_alone("dom"),
    ProbeCase.strict_alone("kot"),
    ProbeCase(
        "nie_ma_plain",
        ("nie", "ma"),
        Phenomenon.CONTROL,
        expected_post_mwt_count=2,
    ),
    ProbeCase.strict_alone("dziewczynka"),
    ProbeCase(
        "ja_idę_do_domu",
        ("ja", "idę", "do", "domu"),
        Phenomenon.CONTROL,
        expected_post_mwt_count=4,
    ),
)
