"""Romanian probe cases (Tier B third batch, 2026-04-23).

Romance with post-posed definite articles. All 5 cases 1-to-1
under with-postprocessor (observed 2026-04-23). ``din`` stays as
one UD word under our postprocessor (despite Stanza's Romanian
MWT being able to split ``de + în`` in free mode).
"""

from __future__ import annotations

from .._probe_types import Phenomenon, ProbeCase


CASES: tuple[ProbeCase, ...] = (
    ProbeCase("casa_alone", ("casă",), Phenomenon.CONTROL, expected_post_mwt_count=1),
    ProbeCase(
        "casa_def_alone",
        ("casa",),
        Phenomenon.CONTROL,
        expected_post_mwt_count=1,
    ),
    ProbeCase(
        "baiatul_alone",
        ("băiatul",),
        Phenomenon.CONTROL,
        expected_post_mwt_count=1,
    ),
    ProbeCase(
        "din_alone",
        ("din",),
        Phenomenon.NATIVE_MWT,
        expected_post_mwt_count=1,
    ),
    ProbeCase(
        "sunt_acasa_plain",
        ("sunt", "acasă"),
        Phenomenon.CONTROL,
        expected_post_mwt_count=2,
    ),
)
