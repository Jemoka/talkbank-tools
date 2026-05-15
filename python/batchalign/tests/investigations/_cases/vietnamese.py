"""Vietnamese probe cases (Tier B third batch, 2026-04-23).

Austroasiatic, monosyllabic with space-per-syllable orthography.
All 5 cases 1-to-1 under with-postprocessor (observed 2026-04-23)
— Vietnamese compounds like ``sinh viên`` (student) stay as two
separate UD words per our pre-tokenized contract, even though
they function as one lexical unit.
"""

from __future__ import annotations

from .._probe_types import Phenomenon, ProbeCase


CASES: tuple[ProbeCase, ...] = (
    ProbeCase("nha_alone", ("nhà",), Phenomenon.CONTROL, expected_post_mwt_count=1),
    ProbeCase("meo_alone", ("mèo",), Phenomenon.CONTROL, expected_post_mwt_count=1),
    ProbeCase(
        "sinh_vien_plain",
        ("sinh", "viên"),
        Phenomenon.CONTROL,
        expected_post_mwt_count=2,
    ),
    ProbeCase(
        "xin_chao_plain",
        ("xin", "chào"),
        Phenomenon.CONTROL,
        expected_post_mwt_count=2,
    ),
    ProbeCase(
        "toi_di_hoc",
        ("tôi", "đi", "học"),
        Phenomenon.CONTROL,
        expected_post_mwt_count=3,
    ),
)
