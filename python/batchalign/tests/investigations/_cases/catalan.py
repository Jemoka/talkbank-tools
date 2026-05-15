"""Catalan probe cases (Tier B pilot, 2026-04-23).

Catalan is typologically close to French / Italian — preposition +
article MWTs (``al`` = a+el, ``del`` = de+el), apostrophe elision
on articles and pronouns (``l'home``, ``d'aquí``, ``s'ha``,
``m'agrada``). BA2-jan9 had no Catalan overrides, so BA3 inherits
Stanza's behavior directly.

Observation (2026-04-23 first golden run): **all cases produce
1-to-1 UD-word counts under the with-postprocessor pipeline**,
including the apostrophe-clitic family. Stanza's Catalan MWT does
split these in free-tokenize mode (e.g. ``l'home`` → ``l'`` +
``home``), but BA3's realignment postprocessor correctly
suppresses the expansion to preserve CHAT's pre-tokenized 1-to-1
contract. This matches the intended behavior for Romance languages
where BA2 had no explicit rules — Stanza's MWT is either
suppressed (apostrophe clitics) or not fired (``al``, ``del``).

Cases assert 1-to-1 under the with-postprocessor path. The
free-tokenize variant is observation-only (no assertion) and
serves as a Stanza-drift sentinel: if Stanza's native MWT behavior
changes, the free-mode output will diff from the pinned value and
the change surfaces for review.
"""

from __future__ import annotations

from .._probe_types import Phenomenon, ProbeCase


CASES: tuple[ProbeCase, ...] = (
    # ── Preposition + article natives (`al`, `del`, `pel`) ─────────
    # Under postprocessor: 1 UD word (Stanza MWT does not fire).
    ProbeCase("al_alone", ("al",), Phenomenon.NATIVE_MWT, expected_post_mwt_count=1),
    ProbeCase("del_alone", ("del",), Phenomenon.NATIVE_MWT, expected_post_mwt_count=1),
    ProbeCase("pel_alone", ("pel",), Phenomenon.NATIVE_MWT, expected_post_mwt_count=1),
    # ── Article / pronoun apostrophe elision ───────────────────────
    # Free-tokenize WOULD split these 1-to-2; postprocessor keeps
    # them 1-to-1 (CHAT pre-tokenization contract).
    ProbeCase(
        "l_home_alone",
        ("l'home",),
        Phenomenon.CLITIC_ELISION,
        expected_post_mwt_count=1,
    ),
    ProbeCase(
        "d_aqui_alone",
        ("d'aquí",),
        Phenomenon.CLITIC_ELISION,
        expected_post_mwt_count=1,
    ),
    ProbeCase(
        "s_ha_alone",
        ("s'ha",),
        Phenomenon.CLITIC_ELISION,
        expected_post_mwt_count=1,
    ),
    ProbeCase(
        "m_agrada_alone",
        ("m'agrada",),
        Phenomenon.CLITIC_ELISION,
        expected_post_mwt_count=1,
    ),
    # ── Plain words (control) ──────────────────────────────────────
    ProbeCase("casa_alone", ("casa",), Phenomenon.CONTROL, expected_post_mwt_count=1),
    ProbeCase(
        "el_noi_plain",
        ("el", "noi"),
        Phenomenon.CONTROL,
        expected_post_mwt_count=2,
    ),
)
