"""RED tests for the token-centric decision-probe API (v2).

v2 replaces integer-indexed per-word addressing (v1) with token-centric
addressing plus n-to-m :class:`TokenMapping` records and
gold-per-side :class:`Gold`. The redesign is motivated by the
2026-04-23 golden-run findings:

* F2 — MWT expansion breaks integer indexing (v1 addressed UD
  words, but case authors wrote indices against pre-tokenized
  input; for contractions the two diverge).
* F3 — POS-only comparator misses semantic regressions (v1 had
  no text/lemma comparison).
* F5 — no n-to-m alignment (period split, period deletion, number
  expansion all need different token counts pre vs post).

These tests drive the v2 design. The v1 tests in
``test_decision_probe_types.py`` will be removed once v2 lands
and the English seed is migrated.

Tests pure-structurally — no Stanza loaded, `StanzaWordResult` /
`StanzaTokenOutput` synthesized directly.
"""

from __future__ import annotations

from dataclasses import FrozenInstanceError

import pytest

from batchalign.tests.investigations._decision_probe_types import (
    DecisionOutcome,
    Gold,
    StanzaTokenOutput,
    StanzaWordResult,
    TokenMapping,
    compare_stanza_outputs,
)


# ─── Helpers ─────────────────────────────────────────────────────────


def _w(text: str, upos: str, lemma: str = "") -> StanzaWordResult:
    return StanzaWordResult(text=text, upos=upos, lemma=lemma or text.lower(), deprel="")


def _tok(text: str, *words: StanzaWordResult) -> StanzaTokenOutput:
    """Synthesize a single-input-token output with one or more UD words.

    For MWT cases pass multiple words: ``_tok("i'll", _w('i','PRON'), _w('will','AUX'))``.
    """
    return StanzaTokenOutput(text=text, words=words)


# ─── Type invariants ─────────────────────────────────────────────────


def test_stanza_token_output_is_frozen() -> None:
    t = _tok("hello", _w("hello", "INTJ"))
    with pytest.raises(FrozenInstanceError):
        t.text = "x"  # type: ignore[misc]


def test_stanza_token_output_carries_multiple_words_for_mwt() -> None:
    """MWT contraction: one input token, multiple UD words."""
    t = _tok("i'll", _w("i", "PRON"), _w("will", "AUX"))
    assert len(t.words) == 2
    assert t.words[0].upos == "PRON"
    assert t.words[1].upos == "AUX"


def test_gold_requires_at_least_one_field_set() -> None:
    """An all-None Gold is unhelpful — the comparator has nothing to
    check. Reject at construction so the mistake is obvious."""
    with pytest.raises(ValueError):
        Gold()


def test_gold_accepts_only_post_side() -> None:
    """A rule that only cares about the post form (pre form's correctness
    is unknown or uninteresting) is valid."""
    g = Gold(post_upos=("PROPN",))
    assert g.post_upos == ("PROPN",)
    assert g.pre_upos is None


def test_token_mapping_rejects_both_sides_empty() -> None:
    """A mapping that affects nothing on either side is a no-op and
    should not be written."""
    with pytest.raises(ValueError):
        TokenMapping(pre_token_indices=(), post_token_indices=(), gold=Gold(post_upos=("X",)))


def test_token_mapping_allows_deletion_shape() -> None:
    """Deletion: pre has tokens, post has none (the period was removed)."""
    m = TokenMapping(
        pre_token_indices=(3,),
        post_token_indices=(),
        gold=Gold(pre_upos=("PUNCT",)),
    )
    assert m.pre_token_indices == (3,)
    assert m.post_token_indices == ()


def test_token_mapping_allows_split_shape() -> None:
    """Split: one pre-token, two post-tokens (e.g. period detach)."""
    m = TokenMapping(
        pre_token_indices=(0,),
        post_token_indices=(0, 1),
        gold=Gold(pre_upos=("PROPN",), post_upos=("PROPN", "PUNCT")),
    )
    assert len(m.pre_token_indices) == 1
    assert len(m.post_token_indices) == 2


# ─── Comparator: 1-to-1 single-word, POS-only ────────────────────────


def test_compare_1to1_both_match_gold_upos() -> None:
    """pre and post both carry PROPN; gold is PROPN on both sides → NEUTRAL."""
    pre = (_tok("Dr.", _w("Dr.", "PROPN")),)
    post = (_tok("Dr", _w("Dr", "PROPN")),)
    mapping = TokenMapping(
        pre_token_indices=(0,),
        post_token_indices=(0,),
        gold=Gold(pre_upos=("PROPN",), post_upos=("PROPN",)),
    )
    result = compare_stanza_outputs(
        pre_tokens=pre,
        post_tokens=post,
        mappings=(mapping,),
    )
    assert result.observed_outcome is DecisionOutcome.POST_NEUTRAL


def test_compare_1to1_post_matches_gold_but_pre_does_not() -> None:
    """Pre has NOUN (wrong), post has PRON (right) → POST_STRICTLY_BETTER."""
    pre = (_tok("i", _w("i", "NOUN")),)
    post = (_tok("I", _w("I", "PRON")),)
    mapping = TokenMapping(
        pre_token_indices=(0,),
        post_token_indices=(0,),
        gold=Gold(pre_upos=("PRON",), post_upos=("PRON",)),
    )
    result = compare_stanza_outputs(pre_tokens=pre, post_tokens=post, mappings=(mapping,))
    assert result.observed_outcome is DecisionOutcome.POST_STRICTLY_BETTER


def test_compare_1to1_pre_matches_but_post_regresses() -> None:
    """Pre is right, post is wrong → POST_STRICTLY_WORSE (a control that
    confirms a rule must not fire)."""
    pre = (_tok("Dr.", _w("Dr.", "PROPN")),)
    post = (_tok("Dr", _w("Dr", "NOUN")),)
    mapping = TokenMapping(
        pre_token_indices=(0,),
        post_token_indices=(0,),
        gold=Gold(pre_upos=("PROPN",), post_upos=("PROPN",)),
    )
    result = compare_stanza_outputs(pre_tokens=pre, post_tokens=post, mappings=(mapping,))
    assert result.observed_outcome is DecisionOutcome.POST_STRICTLY_WORSE


# ─── Comparator: MWT (1-to-1 input, n words per side) ────────────────


def test_compare_mwt_matches_when_both_forms_expand_correctly() -> None:
    """``i'll`` and ``I'll`` both MWT-expand to (PRON, AUX); gold matches
    both sides → NEUTRAL. This is the case v1 got wrong (it sliced the
    UD word at the input-token index, landing on the first UD word only)."""
    pre = (_tok("i'll", _w("i", "PRON"), _w("will", "AUX")),)
    post = (_tok("I'll", _w("I", "PRON"), _w("will", "AUX")),)
    mapping = TokenMapping(
        pre_token_indices=(0,),
        post_token_indices=(0,),
        gold=Gold(pre_upos=("PRON", "AUX"), post_upos=("PRON", "AUX")),
    )
    result = compare_stanza_outputs(pre_tokens=pre, post_tokens=post, mappings=(mapping,))
    assert result.observed_outcome is DecisionOutcome.POST_NEUTRAL


def test_compare_mwt_fails_when_pre_does_not_expand_but_post_does() -> None:
    """If the lowercase contraction isn't MWT-recognized (single UD word)
    but the uppercase one is (two UD words), the gold shapes differ per
    side and only post matches its gold."""
    pre = (_tok("i'll", _w("i'll", "NOUN")),)
    post = (_tok("I'll", _w("I", "PRON"), _w("will", "AUX")),)
    mapping = TokenMapping(
        pre_token_indices=(0,),
        post_token_indices=(0,),
        gold=Gold(pre_upos=("PRON", "AUX"), post_upos=("PRON", "AUX")),
    )
    result = compare_stanza_outputs(pre_tokens=pre, post_tokens=post, mappings=(mapping,))
    # Pre has 1 word for a gold of 2 slots → mismatch. Post matches 2/2.
    assert result.observed_outcome is DecisionOutcome.POST_STRICTLY_BETTER


# ─── Comparator: text-gold (catches DECIMAL_CONTROL semantic loss) ───


def test_compare_text_gold_catches_semantic_regression() -> None:
    """``3.14`` → ``3`` both UPOS=NUM, but post text does not match gold
    text ``3.14``. POS-only comparator returned NEUTRAL (v1 bug); text
    gold returns POST_STRICTLY_WORSE (correct)."""
    pre = (_tok("3.14", _w("3.14", "NUM")),)
    post = (_tok("3", _w("3", "NUM")),)
    mapping = TokenMapping(
        pre_token_indices=(0,),
        post_token_indices=(0,),
        gold=Gold(pre_text=("3.14",), post_text=("3.14",)),
    )
    result = compare_stanza_outputs(pre_tokens=pre, post_tokens=post, mappings=(mapping,))
    assert result.observed_outcome is DecisionOutcome.POST_STRICTLY_WORSE


def test_compare_combined_pos_and_text_gold() -> None:
    """Gold can assert both text and UPOS; both must match for a side
    to be considered correct."""
    pre = (_tok("I", _w("I", "PRON")),)
    post = (_tok("I", _w("I", "PRON")),)
    mapping = TokenMapping(
        pre_token_indices=(0,),
        post_token_indices=(0,),
        gold=Gold(
            pre_upos=("PRON",),
            post_upos=("PRON",),
            pre_text=("I",),
            post_text=("I",),
        ),
    )
    result = compare_stanza_outputs(pre_tokens=pre, post_tokens=post, mappings=(mapping,))
    assert result.observed_outcome is DecisionOutcome.POST_NEUTRAL


# ─── Comparator: n-to-m mappings ─────────────────────────────────────


def test_compare_1to2_period_split() -> None:
    """``Dr. Matthews`` → ``Dr . Matthews``: pre token 0 (``Dr.``) splits
    to post tokens 0 (``Dr``) + 1 (``.``). Gold on pre is 1 UD word
    (PROPN), gold on post is 2 UD words (PROPN, PUNCT)."""
    pre = (
        _tok("Dr.", _w("Dr.", "PROPN")),
        _tok("Matthews", _w("Matthews", "PROPN")),
    )
    post = (
        _tok("Dr", _w("Dr", "PROPN")),
        _tok(".", _w(".", "PUNCT")),
        _tok("Matthews", _w("Matthews", "PROPN")),
    )
    mapping = TokenMapping(
        pre_token_indices=(0,),
        post_token_indices=(0, 1),
        gold=Gold(pre_upos=("PROPN",), post_upos=("PROPN", "PUNCT")),
    )
    result = compare_stanza_outputs(pre_tokens=pre, post_tokens=post, mappings=(mapping,))
    assert result.observed_outcome is DecisionOutcome.POST_NEUTRAL


def test_compare_1to0_deletion() -> None:
    """Pre-token exists and matches gold; post-side is empty (token
    deleted). The mapping describes this with empty post indices.

    Classification: pre matches gold (PUNCT); post is empty. An empty
    post side is NEUTRAL wrt post gold (no post gold is set). So
    overall: pre is fine, post can't contradict → NEUTRAL.
    """
    pre = (
        _tok("him", _w("him", "PRON")),
        _tok(".", _w(".", "PUNCT")),
    )
    post = (_tok("him", _w("him", "PRON")),)
    mapping = TokenMapping(
        pre_token_indices=(1,),
        post_token_indices=(),
        gold=Gold(pre_upos=("PUNCT",)),
    )
    result = compare_stanza_outputs(pre_tokens=pre, post_tokens=post, mappings=(mapping,))
    assert result.observed_outcome is DecisionOutcome.POST_NEUTRAL


def test_compare_1to2_number_expansion_regresses_text() -> None:
    """``23`` → ``twenty three``: 1 pre-token → 2 post-tokens. Pre text
    matches gold ``('23',)``; post text gold ``('23',)`` (single slot)
    doesn't match post's 2 words → REGRESSED.

    This models the rule 'don't expand numbers silently' as a probe."""
    pre = (_tok("23", _w("23", "NUM")),)
    post = (
        _tok("twenty", _w("twenty", "NUM")),
        _tok("three", _w("three", "NUM")),
    )
    mapping = TokenMapping(
        pre_token_indices=(0,),
        post_token_indices=(0, 1),
        gold=Gold(pre_text=("23",), post_text=("23",)),
    )
    result = compare_stanza_outputs(pre_tokens=pre, post_tokens=post, mappings=(mapping,))
    assert result.observed_outcome is DecisionOutcome.POST_STRICTLY_WORSE


# ─── Comparator: multi-mapping aggregation ───────────────────────────


def test_compare_aggregates_mixed_verdicts_across_mappings() -> None:
    """Two mappings — one IMPROVED, one REGRESSED → MIXED at case level."""
    pre = (
        _tok("i", _w("i", "NOUN")),
        _tok("Dr.", _w("Dr.", "PROPN")),
    )
    post = (
        _tok("I", _w("I", "PRON")),
        _tok("Dr", _w("Dr", "NOUN")),
    )
    mapping_i = TokenMapping(
        pre_token_indices=(0,),
        post_token_indices=(0,),
        gold=Gold(pre_upos=("PRON",), post_upos=("PRON",)),
    )
    mapping_dr = TokenMapping(
        pre_token_indices=(1,),
        post_token_indices=(1,),
        gold=Gold(pre_upos=("PROPN",), post_upos=("PROPN",)),
    )
    result = compare_stanza_outputs(
        pre_tokens=pre, post_tokens=post, mappings=(mapping_i, mapping_dr)
    )
    assert result.observed_outcome is DecisionOutcome.MIXED


def test_compare_rejects_out_of_range_indices() -> None:
    """Bad mapping: post_token_indices = (5,) but post_tokens has 1 entry."""
    pre = (_tok("x", _w("x", "NOUN")),)
    post = (_tok("x", _w("x", "NOUN")),)
    mapping = TokenMapping(
        pre_token_indices=(0,),
        post_token_indices=(5,),
        gold=Gold(pre_upos=("NOUN",), post_upos=("NOUN",)),
    )
    with pytest.raises(ValueError):
        compare_stanza_outputs(pre_tokens=pre, post_tokens=post, mappings=(mapping,))


def test_compare_notes_mention_all_mappings() -> None:
    """The notes field summarises every mapping for the reader, even
    when the aggregate verdict is NEUTRAL."""
    pre = (_tok("a", _w("a", "DET")), _tok("b", _w("b", "NOUN")))
    post = (_tok("a", _w("a", "DET")), _tok("b", _w("b", "NOUN")))
    m1 = TokenMapping(
        pre_token_indices=(0,),
        post_token_indices=(0,),
        gold=Gold(pre_upos=("DET",), post_upos=("DET",)),
    )
    m2 = TokenMapping(
        pre_token_indices=(1,),
        post_token_indices=(1,),
        gold=Gold(pre_upos=("NOUN",), post_upos=("NOUN",)),
    )
    result = compare_stanza_outputs(pre_tokens=pre, post_tokens=post, mappings=(m1, m2))
    assert "a" in result.notes and "b" in result.notes
