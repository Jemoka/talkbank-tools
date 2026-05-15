"""Greek probe cases (Tier B second batch, 2026-04-23).

Modern Greek under our with-postprocessor pipeline: all cases
1-to-1. Observation (2026-04-23 first golden run) includes:

* ``σ'αυτό`` stays one UD word (apostrophe clitic doesn't split
  under postprocessor).
* Negation ``δεν``, future particle ``θα`` stay as their own
  tokens (they're already separate in input, no merging).
"""

from __future__ import annotations

from .._probe_types import Phenomenon, ProbeCase


CASES: tuple[ProbeCase, ...] = (
    ProbeCase(
        "spiti_alone",
        ("σπίτι",),
        Phenomenon.CONTROL,
        expected_post_mwt_count=1,
    ),
    ProbeCase(
        "kalimera_alone",
        ("καλημέρα",),
        Phenomenon.CONTROL,
        expected_post_mwt_count=1,
    ),
    ProbeCase(
        "den_exo_plain",
        ("δεν", "έχω"),
        Phenomenon.CONTROL,
        expected_post_mwt_count=2,
    ),
    ProbeCase(
        "tha_pao_plain",
        ("θα", "πάω"),
        Phenomenon.CONTROL,
        expected_post_mwt_count=2,
    ),
    ProbeCase(
        "s_afto_alone",
        ("σ'αυτό",),
        Phenomenon.CLITIC_ELISION,
        expected_post_mwt_count=1,
    ),
)
