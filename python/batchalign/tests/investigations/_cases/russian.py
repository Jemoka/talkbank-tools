"""Russian probe cases (Tier B pilot, 2026-04-23).

Russian's Stanza model **has no MWT processor** — the capability-
aware fixture (`_processors_for()` in conftest.py) drops `mwt` from
the processor list for Russian. All cases stay 1-to-1 both with
and without our postprocessor. BA2-jan9 had no Russian overrides.

These probes serve two purposes:

1. **Capability regression sentinel** — if a future Stanza release
   adds a Russian MWT processor that fires on common words, these
   1-to-1 asserted cases will fail and surface the change.
2. **Tokenization health check** — confirms that the probe harness
   correctly handles a language without MWT support; the earlier
   hardcoded `mwt` processor crashed on Russian (2026-04-23) and
   motivated the capability-aware fixture.
"""

from __future__ import annotations

from .._probe_types import Phenomenon, ProbeCase


CASES: tuple[ProbeCase, ...] = (
    # ── Plain words (control) ──────────────────────────────────────
    ProbeCase("dom_alone", ("дом",), Phenomenon.CONTROL, expected_post_mwt_count=1),
    ProbeCase(
        "sobaka_alone",
        ("собака",),
        Phenomenon.CONTROL,
        expected_post_mwt_count=1,
    ),
    ProbeCase(
        "ya_idu_domoy",
        ("я", "иду", "домой"),
        Phenomenon.CONTROL,
        expected_post_mwt_count=3,
    ),
    # ── Reflexive verb suffix (-ся) — does Stanza tokenizer split? ─
    # No MWT processor for Russian; these stay 1 UD word each.
    ProbeCase(
        "smeyatsya_alone",
        ("смеяться",),
        Phenomenon.POSSESSIVE,
        expected_post_mwt_count=1,
    ),
    ProbeCase(
        "uchitsya_alone",
        ("учится",),
        Phenomenon.POSSESSIVE,
        expected_post_mwt_count=1,
    ),
    # ── Pronoun (control) ──────────────────────────────────────────
    ProbeCase("on_alone", ("он",), Phenomenon.CONTROL, expected_post_mwt_count=1),
)
