"""German probe cases.

Baseline coverage for a language with NO per-language BA2 override
rules. Covers preposition+article contractions (``am = an+dem``,
``im = in+dem``, ``zum = zu+dem``, ``zur = zu+der``, ``beim =
bei+dem``).

2026-04-23 parity audit: locked at observed counts as Stanza-drift
sentinels. Stanza's German MWT coverage is per-token inconsistent:
``am``, ``zur``, ``beim`` stay 1-to-1 under our postprocessor while
``im`` and ``zum`` MWT-expand to 2 UD words. Both in-context cases
expand. Downstream Rust-side Range reassembly collapses back to
1-to-1 for CHAT output, so production parity is preserved.
"""

from __future__ import annotations

from .._probe_types import Phenomenon, ProbeCase


CASES: tuple[ProbeCase, ...] = (
    ProbeCase("am_alone", ("am",), Phenomenon.NATIVE_MWT, 1),
    ProbeCase("im_alone", ("im",), Phenomenon.NATIVE_MWT, 2),
    ProbeCase("zum_alone", ("zum",), Phenomenon.NATIVE_MWT, 2),
    ProbeCase("zur_alone", ("zur",), Phenomenon.NATIVE_MWT, 1),
    ProbeCase("beim_alone", ("beim",), Phenomenon.NATIVE_MWT, 1),
    ProbeCase(
        "am_in_context",
        ("er", "geht", "am", "Morgen"),
        Phenomenon.NATIVE_MWT,
        5,
    ),
    ProbeCase(
        "im_in_context",
        ("das", "Buch", "im", "Zimmer"),
        Phenomenon.NATIVE_MWT,
        5,
    ),
)
