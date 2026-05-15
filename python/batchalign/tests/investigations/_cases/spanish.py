"""Spanish probe cases.

Baseline coverage for a language with NO per-language BA2 override
rules. Covers the two canonical preposition+article contractions
(``al = a + el``, ``del = de + el``).

2026-04-23 parity audit: locked at observed counts as Stanza-drift
sentinels. ``al`` stays 1-to-1 under our postprocessor while ``del``
MWT-expands to 2 UD words; context forms expand. Downstream
Rust-side Range reassembly collapses these back to 1-to-1 for
final CHAT output, so production parity with BA2 is preserved.
"""

from __future__ import annotations

from .._probe_types import Phenomenon, ProbeCase


CASES: tuple[ProbeCase, ...] = (
    ProbeCase("al_alone", ("al",), Phenomenon.NATIVE_MWT, 1),
    ProbeCase("del_alone", ("del",), Phenomenon.NATIVE_MWT, 2),
    ProbeCase(
        "al_in_context",
        ("voy", "al", "cine"),
        Phenomenon.NATIVE_MWT,
        4,
    ),
    ProbeCase(
        "del_in_context",
        ("el", "libro", "del", "niño"),
        Phenomenon.NATIVE_MWT,
        5,
    ),
)
