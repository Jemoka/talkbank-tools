"""Thai probe cases (Tier B third batch, 2026-04-23).

Tai-Kadai, notably unspaced in native orthography. Stanza's Thai
tokenizer handles pre-segmented CHAT input as 1 UD word per input
token. All 5 cases 1-to-1 (observed 2026-04-23).
"""

from __future__ import annotations

from .._probe_types import Phenomenon, ProbeCase


CASES: tuple[ProbeCase, ...] = (
    ProbeCase("ban_alone", ("บ้าน",), Phenomenon.CONTROL, expected_post_mwt_count=1),
    ProbeCase("maeo_alone", ("แมว",), Phenomenon.CONTROL, expected_post_mwt_count=1),
    ProbeCase(
        "chan_pai_ban",
        ("ฉัน", "ไป", "บ้าน"),
        Phenomenon.CONTROL,
        expected_post_mwt_count=3,
    ),
    ProbeCase(
        "sawadee_alone",
        ("สวัสดี",),
        Phenomenon.CONTROL,
        expected_post_mwt_count=1,
    ),
    ProbeCase(
        "nakrian_alone",
        ("นักเรียน",),
        Phenomenon.CONTROL,
        expected_post_mwt_count=1,
    ),
)
