"""Observations: how Stanza handles contracted copula ``is`` (``'s``) in English.

Empirical-grounding tests for the BA3 morphotag Preserve-mode MWT regression
(2026-04-13). Each test isolates one question about Stanza's MWT behavior so
future contributors can see, in one file, exactly what Stanza does for these
constructions in each pipeline mode.

Sentences under test (typical clinical-narrative language samples with
contracted copula ``'s``):
    1. "the stool's going over"      — "stool is going" (copula + progressive)
    2. "and he's falling over"       — "he is falling" (copula + progressive)
    3. "and the sink's overflowing"  — "sink is overflowing" (copula + progressive)
    4. "the lady's washing dishes"   — "lady is washing dishes" (copula + progressive)

All four are the same linguistic phenomenon: contracted copula ``is`` before
a present participle. The CHAT-correct MOR for the clitic in every case is
``aux|be-Fin-Ind-Pres-S3``. We use these tests to pin down WHERE (if
anywhere) that correct analysis is lost in the BA3 pipeline.

Three pipeline configurations are probed:

* ``english_pipeline_free_tokenize`` — Stanza in fully unconstrained mode.
  This is the *baseline* for what Stanza-with-MWT can produce for these
  inputs. If Stanza fails here, the fix is not in BA3 plumbing.
* ``english_pipeline_with_postprocessor`` WITH ``original_words`` set —
  simulates BA3 Preserve-mode (``retokenize=False``). If MWT Range tokens
  are merged back to Single tokens here, we've reproduced the regression
  at the Stanza-layer boundary.
* ``english_pipeline_with_postprocessor`` WITHOUT ``original_words`` —
  simulates BA3 Retokenize-mode. Expected to preserve Range tokens.

See the plan at ``~/.claude/plans/dazzling-singing-reddy.md`` for context.
"""

from __future__ import annotations

import pytest

from batchalign.inference._tokenizer_realign import (
    TokenizerContext,
    make_tokenizer_postprocessor,
)

# Stanza pipeline fixtures are provided by conftest.py in this directory.


# Four copula-contraction sentences, paired with the word lists BA3 would
# hand to ``original_words`` in Preserve mode (each contracted surface kept
# as one CHAT token — "stool's" not "stool" + "'s") AND the OBSERVED
# POS/lemma Stanza currently emits for the contracted ``'s`` component.
#
# The CHAT-correct analysis for every clitic is AUX/be. Stanza gets this
# right for stool/he but commits to a possessive (PART/'s) reading for
# sink/lady — see ``book/src/reference/stanza-limitations.md`` Defect 1.
# BA3 corrects Stanza's wrong call downstream in Rust via the
# ``nlp::invariants::finite_verb_main_clause`` rule. The tests in this
# file document what Stanza itself produces; end-to-end correctness of
# BA3's final ``%mor`` is verified by ``test_preserve_mwt_end_to_end.py``.
COPULA_CONTRACTION_SENTENCES: list[tuple[str, str, list[str], str, str]] = [
    # (label, sentence_text, word_list, expected_clitic_upos, expected_clitic_lemma)
    (
        "stool",
        "the stool's going over .",
        ["the", "stool's", "going", "over", "."],
        "AUX",
        "be",
    ),
    (
        "he",
        "and he's falling over .",
        ["and", "he's", "falling", "over", "."],
        "AUX",
        "be",
    ),
    (
        # Stanza emits PART/'s (wrong). BA3 rewrites to AUX/be downstream.
        "sink",
        "and the sink's overflowing .",
        ["and", "the", "sink's", "overflowing", "."],
        "PART",
        "'s",
    ),
    (
        # Stanza emits PART/'s (wrong). BA3 rewrites to AUX/be downstream.
        "lady",
        "the lady's washing dishes .",
        ["the", "lady's", "washing", "dishes", "."],
        "PART",
        "'s",
    ),
]


# Helper: locate the word(s) corresponding to the contracted token in a
# ``doc.to_dict()`` sentence, regardless of whether Stanza split it.
def _find_contraction(sent: list[dict], head_lemma: str) -> tuple[dict, dict | None]:
    """Return (head_word_dict, clitic_word_dict_or_None).

    Matches the head by lemma (``stool``/``he``/``sink``/``lady``). The
    clitic — if Stanza produced one — is the next word entry in the
    sentence (same Range parent or next Single id).
    """
    for idx, w in enumerate(sent):
        if w.get("lemma", "").lower() == head_lemma:
            head = w
            clitic = sent[idx + 1] if idx + 1 < len(sent) else None
            return head, clitic
    raise AssertionError(
        f"No word with lemma={head_lemma!r} found in sentence "
        f"(lemmas present: {[w.get('lemma') for w in sent]})"
    )


# ---------------------------------------------------------------------------
# Q-A: Free tokenize (Stanza unconstrained) — the semantic baseline
#
# If Stanza's MWT is correct in unconstrained mode, a localized fix in
# BA3's pipeline plumbing is sufficient. If Stanza fails here, the fix
# scope is wider.
# ---------------------------------------------------------------------------


@pytest.mark.golden
class TestFreeTokenizeBaseline:
    """Stanza-with-MWT, unconstrained, MUST split ``X's`` as MWT Range."""

    @pytest.mark.parametrize("label,text,_words,_upos,_lemma", COPULA_CONTRACTION_SENTENCES)
    def test_contraction_becomes_mwt_range(
        self, english_pipeline_free_tokenize, label, text, _words, _upos, _lemma,
    ):
        doc = english_pipeline_free_tokenize(text)
        sent = doc.to_dict()[0]

        # Locate the first appearance of a Range id; this is the MWT token.
        range_entries = [
            w for w in sent
            if isinstance(w.get("id"), (list, tuple)) and len(w["id"]) == 2
        ]
        assert range_entries, (
            f"[{label}] Free tokenize of {text!r} produced NO Range token; "
            f"Stanza's MWT did not fire. Sentence: "
            f"{[(w.get('id'), w.get('text'), w.get('upos')) for w in sent]}"
        )

    @pytest.mark.parametrize(
        "label,text,_words,expected_upos,expected_lemma", COPULA_CONTRACTION_SENTENCES,
    )
    def test_clitic_tagging_matches_observation(
        self, english_pipeline_free_tokenize,
        label, text, _words, expected_upos, expected_lemma,
    ):
        """Encode Stanza's *actual* tagging per-sentence as ground truth.

        Observed 2026-04-13 (Stanza 1.x, GUM MWT package):
          stool's, he's → AUX/be  (copula — correct)
          sink's, lady's → PART/'s (Stanza mis-disambiguates as possessive)

        If a future Stanza upgrade changes any of these, the fixture must
        be updated — that *is* the test's purpose: to make Stanza behavior
        visible in the test suite rather than silently buried.
        """
        doc = english_pipeline_free_tokenize(text)
        sent = doc.to_dict()[0]

        _head, clitic = _find_contraction(sent, label)
        assert clitic is not None, (
            f"[{label}] No clitic word after head lemma {label!r} "
            f"in {text!r}. Sentence: "
            f"{[(w.get('id'), w.get('text'), w.get('upos'), w.get('lemma')) for w in sent]}"
        )
        assert clitic.get("upos") == expected_upos, (
            f"[{label}] Stanza behavior changed? Expected upos={expected_upos!r}, "
            f"got {clitic.get('upos')!r}. If Stanza now disambiguates this "
            f"correctly, update COPULA_CONTRACTION_SENTENCES. "
            f"Full clitic word: {clitic}"
        )
        assert clitic.get("lemma") == expected_lemma, (
            f"[{label}] Stanza behavior changed? Expected lemma={expected_lemma!r}, "
            f"got {clitic.get('lemma')!r}. Full clitic word: {clitic}"
        )

    # Note: a previous iteration of this file had a test asserting Stanza
    # itself should emit AUX/be for sink/lady. That driven-fix test has
    # been removed because BA3 now corrects Stanza's mis-reading via the
    # Rust ``nlp::invariants::finite_verb_main_clause`` rewrite. Stanza
    # itself is still wrong on these cases — that's documented in
    # ``book/src/reference/stanza-limitations.md`` Defect 1. The
    # correctness of BA3's final output is verified end-to-end elsewhere.


# ---------------------------------------------------------------------------
# Q-B: With postprocessor + original_words SET — simulates Preserve mode
#
# Expected (bug reproduction): MWT Range tokens merged back to Single.
# If this is what we observe, we have pinpointed the locus to the
# postprocessor + original_words interaction.
# ---------------------------------------------------------------------------


@pytest.mark.golden
class TestPostprocessorWithOriginalWords:
    """Preserve mode preserves MWT Range tokens through the postprocessor.

    History: before the 2026-04-13 fix to ``_realign_sentence``, this
    class documented the Preserve-mode bug by asserting Range tokens
    were merged back to Single. After the fix, Range tokens survive
    the postprocessor — which is what Rust's ``map_ud_sentence`` needs
    to produce tilde-joined MOR.
    """

    @pytest.mark.parametrize("label,text,words,_upos,_lemma", COPULA_CONTRACTION_SENTENCES)
    def test_contraction_preserved_as_range_token(
        self, english_pipeline_with_postprocessor,
        label, text, words, _upos, _lemma,
    ):
        nlp, ctx = english_pipeline_with_postprocessor
        ctx.original_words = [words]
        try:
            doc = nlp(text)
        finally:
            ctx.original_words = []

        sent = doc.to_dict()[0]

        # The contracted surface (e.g., "stool's") must appear as a Range
        # token (id is a 2-element list/tuple). Without this, Rust never
        # sees MWT structure and cannot emit tilde-joined MOR.
        contracted = f"{label}'s"
        range_entries = [
            w for w in sent
            if (w.get("text") or "").lower() == contracted
            and isinstance(w.get("id"), (list, tuple))
            and len(w["id"]) == 2
        ]
        assert range_entries, (
            f"[{label}] With original_words set, expected a Range entry "
            f"for {contracted!r}; sentence: "
            f"{[(w.get('id'), w.get('text')) for w in sent]}. If this "
            f"fails, _realign_sentence may be stripping Stanza's MWT "
            f"hints again."
        )

    def test_preceding_merge_does_not_strip_following_contraction_range(
        self, english_pipeline_with_postprocessor,
    ):
        """A merged filled-pause token must not kill a later contraction MWT.

        Regression for the observed BA3 output difference on:
        ``mm-hmm that's right .``
        """
        nlp, ctx = english_pipeline_with_postprocessor
        words = ["mm-hmm", "that's", "right", "."]
        ctx.original_words = [words]
        try:
            doc = nlp("mm-hmm that's right .")
        finally:
            ctx.original_words = []

        sent = doc.to_dict()[0]
        thats_entries = [
            w for w in sent
            if (w.get("text") or "").lower() == "that's"
        ]
        assert thats_entries, f"Expected surface token \"that's\" in {sent!r}"
        assert isinstance(thats_entries[0].get("id"), (list, tuple)), (
            "Expected \"that's\" to remain a Range token even when an earlier "
            "token in the sentence required merging. If this fails, "
            "_realign_sentence is still dropping Stanza's native MWT hint "
            "after unrelated merges."
        )


# ---------------------------------------------------------------------------
# Q-C: With postprocessor but WITHOUT original_words — simulates Retokenize
#
# Expected: Range tokens preserved, clitic tagged AUX/lemma=be. This
# documents what BA3 *could* be doing in Preserve if only the
# original_words override were removed.
# ---------------------------------------------------------------------------


@pytest.mark.golden
class TestPostprocessorWithoutOriginalWords:
    """Without original_words, MWT Range tokens survive in the BA3 pipeline.

    This is the state BA3 *should* have in Preserve mode — the
    postprocessor reverts when it has a reason to (retokenize / CJK) but
    the same pipeline object is capable of emitting Range tokens. The bug
    is that the reason-to-merge is always present in Preserve mode.
    """

    @pytest.mark.parametrize("label,text,_words,_upos,_lemma", COPULA_CONTRACTION_SENTENCES)
    def test_contraction_stays_range(
        self, english_pipeline_with_postprocessor,
        label, text, _words, _upos, _lemma,
    ):
        nlp, ctx = english_pipeline_with_postprocessor
        # Explicitly clear — some prior test may have leaked state.
        ctx.original_words = []
        doc = nlp(text)

        sent = doc.to_dict()[0]
        range_entries = [
            w for w in sent
            if isinstance(w.get("id"), (list, tuple)) and len(w["id"]) == 2
        ]
        assert range_entries, (
            f"[{label}] Without original_words, expected a Range token for "
            f"the contraction in {text!r}; got: "
            f"{[(w.get('id'), w.get('text')) for w in sent]}"
        )

    @pytest.mark.parametrize(
        "label,text,_words,expected_upos,expected_lemma", COPULA_CONTRACTION_SENTENCES,
    )
    def test_clitic_tagging_matches_observation(
        self, english_pipeline_with_postprocessor,
        label, text, _words, expected_upos, expected_lemma,
    ):
        """Same tagging observation as the free-tokenize baseline.

        Confirms that running through the postprocessor (without
        original_words) does NOT change Stanza's UPOS/lemma for the
        clitic — it only gates whether Range is preserved. Any
        divergence between this test and the free-tokenize baseline
        would point at the postprocessor doing more than we expect.
        """
        nlp, ctx = english_pipeline_with_postprocessor
        ctx.original_words = []
        doc = nlp(text)

        sent = doc.to_dict()[0]
        _head, clitic = _find_contraction(sent, label)
        assert clitic is not None, (
            f"[{label}] No clitic after head lemma {label!r} in {text!r}. "
            f"Sentence: "
            f"{[(w.get('id'), w.get('text'), w.get('upos'), w.get('lemma')) for w in sent]}"
        )
        assert clitic.get("upos") == expected_upos, (
            f"[{label}] Expected upos={expected_upos!r}, "
            f"got {clitic.get('upos')!r}. Full clitic: {clitic}"
        )
        assert clitic.get("lemma") == expected_lemma, (
            f"[{label}] Expected lemma={expected_lemma!r}, "
            f"got {clitic.get('lemma')!r}. Full clitic: {clitic}"
        )


# ---------------------------------------------------------------------------
# Q-Postproc: Does the tokenize_postprocessor emit the ``(text, True)`` hint
#             for English contractions as documented in ``_tokenizer_realign.py``?
#
# The Rust unit test ``test_english_contraction_merge`` in
# ``tokenizer_realign/mod.rs`` proves ``align_tokens`` returns
# ``PatchedToken::Hint("don't", True)`` when Stanza splits "don't" → "do" +
# "n't" and original_words contains "don't". The Python-side MWT hint tuple
# convention is: ``(text, True)`` signals Stanza's MWT processor to expand
# the merged token. If Q-D (below) shows no Range tokens in the final
# payload, one hypothesis is "the hint never gets emitted". This test
# rules that hypothesis in or out by inspecting the postprocessor's direct
# output for each contraction.
# ---------------------------------------------------------------------------


@pytest.mark.golden
class TestPostprocessorEmitsMWTHint:
    """The postprocessor must emit ``(text, True)`` tuples for contractions.

    If these pass but Q-D (below) still shows no Range tokens, the root
    cause is downstream of the postprocessor — somewhere between the hint
    being set and Stanza's MWT processor not acting on it (or MWT
    running BEFORE the postprocessor, which would make the hint useless).
    """

    @pytest.mark.parametrize(
        "contracted,stanza_split",
        [
            # (surface_as_chat_word, tokens_stanza_would_emit_without_mwt)
            ("stool's", ["stool", "'s"]),
            ("he's", ["he", "'s"]),
            ("sink's", ["sink", "'s"]),
            ("lady's", ["lady", "'s"]),
        ],
    )
    def test_contraction_receives_hint_true(self, contracted, stanza_split):
        ctx = TokenizerContext()
        pp = make_tokenizer_postprocessor(ctx, alpha2="en")
        ctx.original_words = [[contracted]]
        try:
            out = pp([stanza_split])
        finally:
            ctx.original_words = []

        assert len(out) == 1, f"Expected one sentence, got {len(out)}"
        tokens = out[0]
        # The merged contraction should be a (text, True) tuple.
        matches = [
            t for t in tokens
            if isinstance(t, tuple) and t[0] == contracted
        ]
        assert matches, (
            f"Postprocessor did not emit a tuple for {contracted!r}. "
            f"Output: {tokens!r}"
        )
        text, hint = matches[0]
        assert hint is True, (
            f"Postprocessor emitted {text!r} with hint={hint!r}; expected True "
            f"so Stanza's MWT processor re-expands. If this assertion fails, "
            f"the ``(text, True)`` plumbing is broken."
        )


# ---------------------------------------------------------------------------
# Q-D: What does ``batch_infer_morphosyntax`` actually hand to Rust in
#      Preserve mode (``retokenize=False``)?
#
# This is the payload the V2 IPC carries across the Python/Rust boundary.
# Rust's ``map_ud_sentence()`` reads ``raw_sentences`` and decides whether
# to tilde-join. If Range tokens are absent here, no amount of Rust-side
# logic can reconstruct them — the Preserve bug is definitively in
# Python's handling of the request.
# ---------------------------------------------------------------------------


@pytest.mark.golden
class TestBatchInferPreservePayloadToRust:
    """Observe the raw_sentences BA3 sends to Rust for these inputs.

    This bypasses every Rust-side assumption and reveals whether Range
    tokens make it to the IPC boundary. If they do not, the fix must be
    on the Python side.
    """

    @pytest.mark.parametrize("label,text,words,_upos,_lemma", COPULA_CONTRACTION_SENTENCES)
    def test_preserve_payload_has_range_tokens(
        self, english_pipeline_with_postprocessor,
        label, text, words, _upos, _lemma,
    ):
        """Post-fix behavior: ``retokenize=False`` preserves MWT structure.

        The ``raw_sentences[0][*]['id']`` for the contracted token is a
        ``[1, 2]`` Range pair (list, since JSON lacks tuples), followed
        by its component words with Single ids. This is what
        ``map_ud_sentence()`` in Rust consumes to emit tilde-joined MOR.

        History: before the 2026-04-13 fix, this test asserted the
        INVERSE — that Range tokens were absent from the payload (the
        bug). After the fix, Range tokens survive, so the assertion is
        inverted.
        """
        import threading

        from batchalign.inference.morphosyntax import batch_infer_morphosyntax
        from batchalign.worker._types import BatchInferRequest

        nlp, ctx = english_pipeline_with_postprocessor

        req = BatchInferRequest(
            task="morphosyntax",
            items=[{
                "words": words,
                "terminator": ".",
                "special_forms": [[None, None]] * len(words),
                "lang": "eng",
            }],
            lang="eng",
            retokenize=False,
            mwt={},
        )

        response = batch_infer_morphosyntax(
            req,
            nlp_pipelines={"eng": nlp},
            contexts={"eng": ctx},
            nlp_lock=threading.Lock(),
            free_threaded=False,
        )

        result = response.results[0].result
        raw_sentences = result.get("raw_sentences", [[]])
        first_sent = raw_sentences[0]

        # The Range entry whose surface matches the contracted token.
        contracted = f"{label}'s"
        range_entries = [
            w for w in first_sent
            if (w.get("text") or "").lower() == contracted
            and isinstance(w.get("id"), (list, tuple))
            and len(w["id"]) == 2
        ]
        assert range_entries, (
            f"[{label}] Expected a Range entry for {contracted!r} in the "
            f"Preserve payload — this is what Rust needs to tilde-join. "
            f"Sentence: {[(w.get('id'), w.get('text')) for w in first_sent]}. "
            f"If this fails, the Preserve-mode MWT regression has returned."
        )
