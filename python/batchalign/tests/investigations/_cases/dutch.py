"""Dutch probe cases.

Covers the ``EndsWith("'s") → SuppressMwt`` rule target:

* Proper-name possessives (``Claus's``, ``Maria's``, ``Jan's``).
* Pseudo-contractions (``het's``, ``er's``).
* Hyphenated time idioms (``'s-avonds``, ``'s-morgens``).
* Short contractions (``'t``, ``'n``).
* Control nouns (``huis``).

All with-postprocessor cases are strict 1-to-1. The 13-case audit
showed Stanza Dutch emits these as 1 UD word each; the old
SuppressMwt rule was dormant.
"""

from __future__ import annotations

from .._probe_types import Phenomenon, ProbeCase


CASES: tuple[ProbeCase, ...] = (
    # ── Proper-name possessives ──
    ProbeCase("claus_s_possessive", ("Claus's",), Phenomenon.POSSESSIVE, 1),
    ProbeCase("maria_s_possessive", ("Maria's",), Phenomenon.POSSESSIVE, 1),
    ProbeCase("jan_s_possessive", ("Jan's",), Phenomenon.POSSESSIVE, 1),
    # ── Pseudo-contractions (ess clitic) ──
    ProbeCase("het_s_contraction", ("het's",), Phenomenon.CONTRACTION, 1),
    ProbeCase("er_s_contraction", ("er's",), Phenomenon.CONTRACTION, 1),
    # ── 's-avonds / 's-morgens time idioms ──
    ProbeCase("s_avonds_joined", ("'s-avonds",), Phenomenon.POSSESSIVE, 1),
    ProbeCase("s_morgens_joined", ("'s-morgens",), Phenomenon.POSSESSIVE, 1),
    # ── 't / 'n short contractions (related class) ──
    ProbeCase("apostrophe_t_alone", ("'t",), Phenomenon.CONTRACTION, 1),
    ProbeCase("apostrophe_n_alone", ("'n",), Phenomenon.CONTRACTION, 1),
    # ── Control words ──
    ProbeCase("plain_dutch_word", ("huis",), Phenomenon.CONTROL, 1),
    ProbeCase(
        "plain_word_ending_in_s",
        ("huis_is",),
        Phenomenon.CONTROL,
        1,
    ),
    # ── Sentence contexts ──
    ProbeCase(
        "claus_s_in_context",
        ("dat", "is", "Claus's", "hond"),
        Phenomenon.POSSESSIVE,
        4,
    ),
    ProbeCase(
        "s_avonds_in_context",
        ("hij", "komt", "'s-avonds", "terug"),
        Phenomenon.POSSESSIVE,
        4,
    ),
    ProbeCase(
        "apostrophe_t_in_context",
        ("'t", "is", "koud"),
        Phenomenon.CONTRACTION,
        3,
    ),
)
