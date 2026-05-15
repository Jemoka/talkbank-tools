"""Hebrew probe cases (Tier B second batch, 2026-04-23).

Hebrew is Semitic, RTL, fusional prefixes (``ה`` article, ``ב``
preposition, ``ו`` conjunction, etc.).

Observation (2026-04-23 first golden run): **Hebrew is the only
Tier B language where Stanza's MWT processor fires under our
with-postprocessor pipeline**, producing 1-to-2 or 1-to-3 UD-word
splits where every other Tier B language (Arabic, Catalan, Greek,
Polish, Russian, Turkish, Swedish, Indonesian) stays 1-to-1.

Concretely:

* ``הבית`` (ha-bayit) → 2 UD words (``ה`` DET + ``בית`` NOUN)
* ``בבית`` (ba-bayit) → 2 UD words (``ב`` ADP + ``בית`` NOUN)
* ``והספר`` (ve-ha-sefer) → 3 UD words (``ו`` + ``ה`` + ``ספר``)

This asymmetry with Arabic (same family, same fusional-prefix
morphology, but Stanza MWT doesn't fire) is a real typological
finding. For CHAT files where input is pre-tokenized at the
orthographic-word level (e.g., ``הבית`` as one CHAT word),
Stanza producing 2 UD words violates the 1-to-1 contract BA3
relies on downstream.

**Potential latent gap.** This may be a real pipeline concern for
Hebrew corpora. Cases below lock Stanza's OBSERVED count (not the
CHAT-expected 1-to-1) so the current behavior is pinned. If Hebrew
corpora are added to production and the 1-to-n mismatch causes
downstream issues, the pipeline likely needs a Hebrew-specific
postprocessor override similar to what BA2 had for French clitics.

Future Stanza upgrades that change Hebrew MWT behavior will fail
these asserted counts and surface for re-adjudication.
"""

from __future__ import annotations

from .._probe_types import Phenomenon, ProbeCase


CASES: tuple[ProbeCase, ...] = (
    # ── Plain words (control) ──────────────────────────────────────
    ProbeCase(
        "bayit_alone",
        ("בית",),
        Phenomenon.CONTROL,
        expected_post_mwt_count=1,
    ),
    ProbeCase(
        "sefer_alone",
        ("ספר",),
        Phenomenon.CONTROL,
        expected_post_mwt_count=1,
    ),
    # ── Article prefix (ha- + noun) — Stanza MWT fires ─────────────
    ProbeCase(
        "ha_bayit_alone",
        ("הבית",),
        Phenomenon.NATIVE_MWT,
        expected_post_mwt_count=2,
    ),
    ProbeCase(
        "ha_sefer_alone",
        ("הספר",),
        Phenomenon.NATIVE_MWT,
        expected_post_mwt_count=2,
    ),
    # ── Preposition + article + noun — Stanza MWT fires ────────────
    ProbeCase(
        "ba_bayit_alone",
        ("בבית",),
        Phenomenon.NATIVE_MWT,
        expected_post_mwt_count=2,
    ),
    ProbeCase(
        "la_bayit_alone",
        ("לבית",),
        Phenomenon.NATIVE_MWT,
        expected_post_mwt_count=2,
    ),
    # ── Conjunction + article + noun — 3-way expansion ─────────────
    ProbeCase(
        "ve_ha_sefer_alone",
        ("והספר",),
        Phenomenon.NATIVE_MWT,
        expected_post_mwt_count=3,
    ),
    # ── Sentence context ───────────────────────────────────────────
    # 2 CHAT words → 3 UD (because ba_bayit expands to 2).
    ProbeCase(
        "ani_ba_bayit",
        ("אני", "בבית"),
        Phenomenon.CONTROL,
        expected_post_mwt_count=3,
    ),
)
