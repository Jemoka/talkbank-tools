"""French probe cases.

Covers the four phenomena that the 2026-04-21 MWT-override audit
examined for French:

* Elision-prefix words (``jusqu'à``, ``puisqu'il``, ``quelqu'un``) —
  the Wave 4 invariant-break family. Strict 1-to-1.
* Multi-clitic stacks (``d'l'attraper``) — same invariant family.
* Clitic-apostrophe contractions (``c'est``, ``qu'il``, ``l'ami``) —
  strict 1-to-1, protected from regression.
* ``aujourd'hui`` in various positions — strict 1-to-1 to confirm the
  removed PlainText rule is not load-bearing.
* Preposition+article natives (``au``, ``du``, ``des``) — observe
  only; Stanza's MWT expansion is the intended behavior and the
  Rust-side Range reassembly collapses post-expansion counts back.

The seed case ``seed_040802_1620`` pins the specific absorbed-failure
utterance from ``phon-eng-french-data/French/Paris/Antoine/040802.cha``
line 1620.
"""

from __future__ import annotations

from .._probe_types import Phenomenon, ProbeCase


SEED_040802_1620_WORDS: tuple[str, ...] = (
    "euh",
    "oui",
    "mais",
    "de",
    "toute",  # from CHAT t(oute), cleaned by Rust before reaching Stanza
    "façon",
    "c'est",
    "ouvert",
    "jusqu'à",
)


def _strict(words: tuple[str, ...]) -> int:
    """Shorthand: strict 1-to-1 expected count = len(words)."""
    return len(words)


CASES: tuple[ProbeCase, ...] = (
    # ── Seed utterance from the Wave 4 absorbed-failure investigation ──
    ProbeCase(
        label="seed_040802_1620",
        words=SEED_040802_1620_WORDS,
        phenomenon=Phenomenon.ELISION_PREFIX,
        expected_post_mwt_count=_strict(SEED_040802_1620_WORDS),
    ),
    # ── Elision prefix family (the invariant-break root cause) ──
    ProbeCase(
        label="quelqu_un_alone",
        words=("quelqu'un",),
        phenomenon=Phenomenon.ELISION_PREFIX,
        expected_post_mwt_count=1,
    ),
    ProbeCase(
        label="jusqu_a_alone",
        words=("jusqu'à",),
        phenomenon=Phenomenon.ELISION_PREFIX,
        expected_post_mwt_count=1,
    ),
    ProbeCase(
        label="jusqu_en_alone",
        words=("jusqu'en",),
        phenomenon=Phenomenon.ELISION_PREFIX,
        expected_post_mwt_count=1,
    ),
    ProbeCase(
        label="puisqu_il_alone",
        words=("puisqu'il",),
        phenomenon=Phenomenon.ELISION_PREFIX,
        expected_post_mwt_count=1,
    ),
    ProbeCase(
        label="quelqu_un_in_context",
        words=("il", "a", "rencontré", "quelqu'un", "."),
        phenomenon=Phenomenon.ELISION_PREFIX,
        expected_post_mwt_count=5,
    ),
    ProbeCase(
        label="jusqu_a_in_context",
        words=("elle", "a", "attendu", "jusqu'à", "minuit", "."),
        phenomenon=Phenomenon.ELISION_PREFIX,
        expected_post_mwt_count=6,
    ),
    ProbeCase(
        label="multi_clitic_triple",
        words=("d'l'attraper",),
        phenomenon=Phenomenon.MULTI_CLITIC,
        expected_post_mwt_count=1,
    ),
    # ── Protected contractions (must round-trip) ──
    ProbeCase("c_est_alone", ("c'est",), Phenomenon.CLITIC_ELISION, 1),
    ProbeCase("qu_il_alone", ("qu'il",), Phenomenon.CLITIC_ELISION, 1),
    ProbeCase("l_ami_alone", ("l'ami",), Phenomenon.CLITIC_ELISION, 1),
    ProbeCase("d_un_alone", ("d'un",), Phenomenon.CLITIC_ELISION, 1),
    ProbeCase("n_avait_alone", ("n'avait",), Phenomenon.CLITIC_ELISION, 1),
    ProbeCase("s_il_alone", ("s'il",), Phenomenon.CLITIC_ELISION, 1),
    ProbeCase("t_as_alone", ("t'as",), Phenomenon.CLITIC_ELISION, 1),
    ProbeCase("m_a_alone", ("m'a",), Phenomenon.CLITIC_ELISION, 1),
    ProbeCase("j_ai_alone", ("j'ai",), Phenomenon.CLITIC_ELISION, 1),
    ProbeCase("c_est_in_context", ("c'est", "ouvert"), Phenomenon.CLITIC_ELISION, 2),
    ProbeCase(
        "qu_il_in_context",
        ("il", "dit", "qu'il", "part"),
        Phenomenon.CLITIC_ELISION,
        4,
    ),
    ProbeCase(
        "l_ami_in_context",
        ("voici", "l'ami", "de", "Marie"),
        Phenomenon.CLITIC_ELISION,
        4,
    ),
    ProbeCase(
        "s_il_in_context",
        ("s'il", "vient", "demain"),
        Phenomenon.CLITIC_ELISION,
        3,
    ),
    ProbeCase(
        "j_ai_in_context",
        ("j'ai", "vu", "Paul"),
        Phenomenon.CLITIC_ELISION,
        3,
    ),
    # ── aujourd'hui in various positions ──
    ProbeCase(
        "aujourdhui_alone",
        ("aujourd'hui",),
        Phenomenon.IDIOMATIC,
        1,
    ),
    ProbeCase(
        "aujourdhui_sentence_start",
        ("aujourd'hui", "il", "pleut"),
        Phenomenon.IDIOMATIC,
        3,
    ),
    ProbeCase(
        "aujourdhui_sentence_end",
        ("il", "pleut", "aujourd'hui"),
        Phenomenon.IDIOMATIC,
        3,
    ),
    ProbeCase(
        "aujourdhui_embedded",
        ("je", "pense", "aujourd'hui", "à", "toi"),
        Phenomenon.IDIOMATIC,
        5,
    ),
    # ── Native MWT controls (2026-04-23 parity audit: locked as
    #    Stanza-drift sentinels at observed counts; MWT Range
    #    reassembly downstream collapses them back to 1-to-1 for
    #    CHAT output, but raw Stanza counts vary per token) ──
    ProbeCase("du_alone", ("du",), Phenomenon.NATIVE_MWT, 1),
    ProbeCase("au_alone", ("au",), Phenomenon.NATIVE_MWT, 1),
    ProbeCase("aux_alone", ("aux",), Phenomenon.NATIVE_MWT, 2),
    ProbeCase("des_alone", ("des",), Phenomenon.NATIVE_MWT, 2),
    ProbeCase(
        "du_in_context",
        ("j'ai", "pris", "du", "pain", "."),
        Phenomenon.NATIVE_MWT,
        6,
    ),
    ProbeCase(
        "au_in_context",
        ("il", "va", "au", "marché", "."),
        Phenomenon.NATIVE_MWT,
        6,
    ),
)
