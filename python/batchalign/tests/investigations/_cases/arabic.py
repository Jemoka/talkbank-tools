"""Arabic probe cases (Tier B pilot, 2026-04-23).

Arabic is heavily fusional at the orthographic level — prepositions,
articles, conjunctions, and pronouns attach as prefixes/suffixes
to a host token. Observation (2026-04-23 first golden run): **all
cases produce 1-to-1 UD-word counts under both free-tokenize and
with-postprocessor paths**. Stanza's Arabic MWT either has no model
or the model does not fire on these common article-preposition-
conjunction fusions.

Concretely observed:

* ``والكتاب`` (wa-al-kitāb, "and the book" — 3 morphemes) → stays
  as 1 UD word, NOT expanded to 3.
* ``بالبيت`` (bi-al-bayt, "in the house" — 3 morphemes) → stays
  as 1 UD word, tagged X (Stanza's UPOS fallback).
* Plain words tokenize 1-to-1.

This is the first RTL-script probe in the harness. It confirms that
RTL text round-trips correctly through the fixture infrastructure
and the test-ID serialization.

Adjudication: for CHAT input where a whole fused word is one
pre-tokenized unit, 1-to-1 is the correct behavior — our pipeline
matches what CHAT expects. If future Stanza upgrades add Arabic
MWT expansion that fires on these inputs, these asserted counts
will fail and surface the change for re-adjudication.
"""

from __future__ import annotations

from .._probe_types import Phenomenon, ProbeCase


CASES: tuple[ProbeCase, ...] = (
    # ── Plain words (control; no clitics attached) ─────────────────
    ProbeCase(
        "kitab_alone",
        ("كتاب",),
        Phenomenon.CONTROL,
        expected_post_mwt_count=1,
    ),
    ProbeCase(
        "bayt_alone",
        ("بيت",),
        Phenomenon.CONTROL,
        expected_post_mwt_count=1,
    ),
    # ── Article clitic (al- + noun) ────────────────────────────────
    ProbeCase(
        "al_kitab_alone",
        ("الكتاب",),
        Phenomenon.NATIVE_MWT,
        expected_post_mwt_count=1,
    ),
    ProbeCase(
        "al_bayt_alone",
        ("البيت",),
        Phenomenon.NATIVE_MWT,
        expected_post_mwt_count=1,
    ),
    # ── Conjunction + article + noun (wa + al + kitāb) ─────────────
    ProbeCase(
        "wa_al_kitab_alone",
        ("والكتاب",),
        Phenomenon.NATIVE_MWT,
        expected_post_mwt_count=1,
    ),
    # ── Preposition + article + noun ───────────────────────────────
    ProbeCase(
        "bi_al_bayt_alone",
        ("بالبيت",),
        Phenomenon.NATIVE_MWT,
        expected_post_mwt_count=1,
    ),
    ProbeCase(
        "li_al_walad_alone",
        ("للولد",),
        Phenomenon.NATIVE_MWT,
        expected_post_mwt_count=1,
    ),
    # ── Simple sentence context ────────────────────────────────────
    ProbeCase(
        "ana_fi_al_bayt",
        ("أنا", "في", "البيت"),
        Phenomenon.CONTROL,
        expected_post_mwt_count=3,
    ),
)
