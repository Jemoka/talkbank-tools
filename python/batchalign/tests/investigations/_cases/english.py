"""English MWT probe cases.

Covers the phenomena BA2-jan9 explicitly gated in its English
apostrophe-contraction rule (``ud.py:694-697``):

* Standard contractions (``don't``, ``I'm``, ``can't``,
  ``you're``, ``it's``, ``they've``, ``we'd``, ``she'll``,
  ``won't``, ``isn't``). Stanza's English MWT processor expands
  these to 2 UD words; we assert the count so a regression
  would surface.
* Possessives (``John's``, ``dog's``). Stanza treats possessive
  ``'s`` as a separate UD word via MWT expansion.
* ``o'clock`` — BA2's explicit control. The apostrophe rule did
  NOT fire on ``o'`` + word, so ``o'clock`` stayed unsplit.
  Probe asserts Stanza produces 1 UD word.
* ``gonna`` — Stanza's English MWT expands this to ``gon + na``
  (or similar) even without an apostrophe. Assert 2 UD words.
* Native MWT baselines — short controls to confirm the English
  pipeline is otherwise healthy.

BA2 rule origin
---------------
BA2 ``tokenizer_processor()`` line 694-697::

    elif (("en" in lang) and matches_in(i, "'") and
          not (len(conform(i).split("'")) > 1 and
               conform(i).split("'")[0].strip() == "o")):
        res.append((conform(i), True))

The ``True`` flag told BA2's downstream morphology pass to join
the contraction back together after Stanza split it. BA3 has no
equivalent; Stanza's native MWT processor is expected to handle
these cases. These probes confirm whether Stanza does.

Observe-only today
------------------
All counts below reflect what Stanza *should* emit based on linguistic
expectation. A first golden run will pin what Stanza actually does;
if the observed count matches the expected, the probe locks. If it
diverges, that's a BA2-parity regression to adjudicate.
"""

from __future__ import annotations

from .._probe_types import Phenomenon, ProbeCase


CASES: tuple[ProbeCase, ...] = (
    # ── Standard contractions (MWT should expand to 2 UD words) ─────
    ProbeCase(
        label="dont_alone",
        words=("don't",),
        phenomenon=Phenomenon.CONTRACTION,
        expected_post_mwt_count=2,
    ),
    ProbeCase(
        label="im_alone",
        words=("I'm",),
        phenomenon=Phenomenon.CONTRACTION,
        expected_post_mwt_count=2,
    ),
    ProbeCase(
        label="cant_alone",
        words=("can't",),
        phenomenon=Phenomenon.CONTRACTION,
        expected_post_mwt_count=2,
    ),
    ProbeCase(
        label="youre_alone",
        words=("you're",),
        phenomenon=Phenomenon.CONTRACTION,
        expected_post_mwt_count=2,
    ),
    ProbeCase(
        label="its_alone",
        words=("it's",),
        phenomenon=Phenomenon.CONTRACTION,
        expected_post_mwt_count=2,
    ),
    ProbeCase(
        label="theyve_alone",
        words=("they've",),
        phenomenon=Phenomenon.CONTRACTION,
        expected_post_mwt_count=2,
    ),
    ProbeCase(
        label="wed_alone",
        words=("we'd",),
        phenomenon=Phenomenon.CONTRACTION,
        expected_post_mwt_count=2,
    ),
    ProbeCase(
        label="shell_alone",
        words=("she'll",),
        phenomenon=Phenomenon.CONTRACTION,
        expected_post_mwt_count=2,
    ),
    ProbeCase(
        label="wont_alone",
        words=("won't",),
        phenomenon=Phenomenon.CONTRACTION,
        expected_post_mwt_count=2,
    ),
    ProbeCase(
        label="isnt_alone",
        words=("isn't",),
        phenomenon=Phenomenon.CONTRACTION,
        expected_post_mwt_count=2,
    ),
    # ── Contraction in context ─────────────────────────────────────
    ProbeCase(
        label="dont_in_context",
        # 4 CHAT words → expect 5 UD words (don't expands to 2)
        words=("I", "don't", "know", "that"),
        phenomenon=Phenomenon.CONTRACTION,
        expected_post_mwt_count=5,
    ),
    # ── Possessives (MWT-expansion family, 2 UD words) ─────────────
    ProbeCase(
        label="johns_possessive",
        words=("John's", "book"),
        phenomenon=Phenomenon.POSSESSIVE,
        expected_post_mwt_count=3,
    ),
    ProbeCase(
        label="dogs_possessive",
        words=("the", "dog's", "tail"),
        phenomenon=Phenomenon.POSSESSIVE,
        expected_post_mwt_count=4,
    ),
    # ── Compound MWT (Stanza expands even without apostrophe) ──────
    ProbeCase(
        label="gonna_alone",
        words=("gonna",),
        phenomenon=Phenomenon.CONTRACTION,
        expected_post_mwt_count=2,
    ),
    ProbeCase(
        label="gonna_in_context",
        words=("I'm", "gonna", "go"),
        # I'm → 2, gonna → 2, go → 1: total 5 UD
        phenomenon=Phenomenon.CONTRACTION,
        expected_post_mwt_count=5,
    ),
    # ── o'clock control (BA2 explicitly excluded — must NOT expand) ─
    ProbeCase(
        label="oclock_alone",
        words=("o'clock",),
        phenomenon=Phenomenon.CONTROL,
        expected_post_mwt_count=1,
    ),
    ProbeCase(
        label="oclock_in_context",
        words=("at", "six", "o'clock"),
        phenomenon=Phenomenon.CONTROL,
        expected_post_mwt_count=3,
    ),
    # ── Native MWT / plain-word controls ───────────────────────────
    ProbeCase(
        label="hello_alone",
        words=("hello",),
        phenomenon=Phenomenon.CONTROL,
        expected_post_mwt_count=1,
    ),
    ProbeCase(
        label="the_dog_plain",
        words=("the", "dog", "ran"),
        phenomenon=Phenomenon.CONTROL,
        expected_post_mwt_count=3,
    ),
)
