"""Korean probe cases (Tier B third batch, 2026-04-23).

Koreanic, agglutinative with particles. All 5 cases 1-to-1 under
with-postprocessor (observed 2026-04-23) — Stanza's Korean
tokenizer keeps particles (``은/는``, ``이/가``, ``을/를``, ``에``)
attached to their host nouns as one UD word.

Note: the free-tokenize path may split these particles; the
with-postprocessor path preserves our pre-tokenized 1-to-1
contract.
"""

from __future__ import annotations

from .._probe_types import Phenomenon, ProbeCase


CASES: tuple[ProbeCase, ...] = (
    ProbeCase("jib_alone", ("집",), Phenomenon.CONTROL, expected_post_mwt_count=1),
    ProbeCase("chaek_alone", ("책",), Phenomenon.CONTROL, expected_post_mwt_count=1),
    ProbeCase(
        "jib_e_alone",
        ("집에",),
        Phenomenon.NATIVE_MWT,
        expected_post_mwt_count=1,
    ),
    ProbeCase(
        "chaek_eul_alone",
        ("책을",),
        Phenomenon.NATIVE_MWT,
        expected_post_mwt_count=1,
    ),
    ProbeCase(
        "na_neun_jib_e_kanda",
        ("나는", "집에", "간다"),
        Phenomenon.CONTROL,
        expected_post_mwt_count=3,
    ),
)
