"""Portuguese probe cases.

Covers:

* The idiomatic elision ``d'água`` alone and in context — the old
  BA2-jan9 ``Exact("d'água") → ForceMwt`` target.
* Preposition+article natives (``do``, ``da``, ``na``).

2026-04-23 parity audit: all cases produce 1-to-1 under our
postprocessor (Stanza's Portuguese MWT is suppressed for
preposition+article and idiomatic contractions). Locked to
observed counts as Stanza-drift sentinels. The BA2 ``ForceMwt``
rule for ``d'água`` is thus retired with evidence: BA3 achieves
the same single-UD-word outcome natively.
"""

from __future__ import annotations

from .._probe_types import Phenomenon, ProbeCase


CASES: tuple[ProbeCase, ...] = (
    # ── d'água idiomatic elision (BA2 ForceMwt target; now 1-to-1) ─
    ProbeCase(
        "d_agua_alone",
        ("d'água",),
        Phenomenon.IDIOMATIC,
        expected_post_mwt_count=1,
    ),
    ProbeCase(
        "d_agua_in_context",
        ("copo", "d'água", "frio"),
        Phenomenon.IDIOMATIC,
        expected_post_mwt_count=3,
    ),
    # ── Native MWT controls ──
    ProbeCase("do_alone", ("do",), Phenomenon.NATIVE_MWT, 1),
    ProbeCase("da_alone", ("da",), Phenomenon.NATIVE_MWT, 1),
    ProbeCase("na_alone", ("na",), Phenomenon.NATIVE_MWT, 1),
)
