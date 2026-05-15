"""RED tests: BA3 Preserve mode must preserve Stanza MWT Range tokens.

The 2026-04-13 Preserve-mode regression: ``batchalign3 morphotag`` without
``--retokenize`` produces ``noun|stool-Plur``, ``noun|he-Plur``, etc.
instead of BA2's tilde-joined MWT analysis. Phase 0 empirical matrix (see
``test_stanza_mwt_copula_observations.py``) pinpointed the locus: in
Preserve mode (``retokenize=False``), BA3's Python pipeline merges
Stanza's MWT Range tokens back to Single tokens before they cross the V2
IPC boundary, so Rust's ``map_ud_sentence()`` never gets to tilde-join
them.

These tests encode the DESIRED behavior — they will be RED until the
Preserve-mode MWT regression is fixed, then GREEN afterwards. They
serve as permanent regression guards.

Target assertions (from Phase 0 decision gate):

* For every contraction under test, ``batch_infer_morphosyntax`` must
  return ``raw_sentences`` containing a Range entry (``id = [n, n+1]``)
  and a clitic component word — i.e., Rust receives MWT-expanded UD data.
* The tilde-joined ``%mor`` target downstream (tested at a higher layer
  when the fix lands) is:

  | Surface   | %mor clitic |
  |-----------|-------------|
  | stool's   | noun\\|stool~aux\\|be-Fin-Ind-Pres-S3 |
  | he's      | pron\\|he-...~aux\\|be-Fin-Ind-Pres-S3 |
  | sink's    | noun\\|sink~part\\|s       (Stanza mis-tags; see Phase 0 xfails) |
  | lady's    | noun\\|lady~part\\|s       (Stanza mis-tags; see Phase 0 xfails) |

  All four show the critical tilde join, which is the property that
  identifies "BA3 tilde-joined MWT analysis is present" vs. "BA3 emits
  degraded single-token MORs".
"""

from __future__ import annotations

import pytest

# Stanza pipeline fixtures are provided by conftest.py in this directory.


# (label, sentence_text_for_stanza, word_list_for_CHAT, head_lemma)
# — ``head_lemma`` is the lemma of the noun/pronoun that precedes ``'s``.
#
# These mirror the ``COPULA_CONTRACTION_SENTENCES`` fixture in
# ``test_stanza_mwt_copula_observations.py`` but carry only the fields
# this file needs — the Phase 0 file is the source of truth for expected
# Stanza behavior.
COPULA_CONTRACTION_SENTENCES = [
    ("stool", ["the", "stool's", "going", "over", "."], "stool"),
    ("he", ["and", "he's", "falling", "over", "."], "he"),
    ("sink", ["and", "the", "sink's", "overflowing", "."], "sink"),
    ("lady", ["the", "lady's", "washing", "dishes", "."], "lady"),
]


def _build_req(words: list[str]) -> "BatchInferRequest":  # noqa: UP037
    """Build a Preserve-mode (``retokenize=False``) request.

    Import is local to keep module-level imports light — this function
    is only called inside a test body.
    """
    from batchalign.worker._types import BatchInferRequest

    return BatchInferRequest(
        task="morphosyntax",
        items=[{
            "words": words,
            "terminator": ".",
            "special_forms": [[None, None]] * len(words),
            "lang": "eng",
        }],
        lang="eng",
        retokenize=False,  # Preserve mode — the bug path
        mwt={},
    )


def _first_sentence_raw(response) -> list[dict]:
    """Pull ``raw_sentences[0]`` from a ``batch_infer_morphosyntax`` response."""
    result = response.results[0].result
    raw = result.get("raw_sentences", [[]])
    return raw[0] if raw else []


def _find_contracted_chat_position(words: list[str], head_lemma: str) -> int:
    """Return the 1-based CHAT position of the contracted token (``X's``)."""
    contracted = f"{head_lemma}'s"
    for idx, w in enumerate(words):
        if w.lower() == contracted:
            return idx + 1  # CHAT positions are 1-based for Stanza
    raise AssertionError(
        f"No token {contracted!r} in word list {words}"
    )


# ---------------------------------------------------------------------------
# RED #1: Range tokens must appear in the IPC payload for every contraction.
# ---------------------------------------------------------------------------


@pytest.mark.golden
class TestPreserveIPCEmitsRangeTokens:
    """Preserve mode must hand MWT Range tokens to Rust.

    When these pass, Rust's ``map_ud_sentence()`` has the input it needs
    to tilde-join the clitic. When they fail (current state), Rust sees
    merged Single tokens and cannot reconstruct MWT analysis — the
    observable symptom of the Preserve-mode MWT regression.
    """

    @pytest.mark.parametrize("label,words,head_lemma", COPULA_CONTRACTION_SENTENCES)
    def test_payload_contains_range_entry_for_contraction(
        self, english_pipeline_with_postprocessor,
        label, words, head_lemma,
    ):
        import threading

        from batchalign.inference.morphosyntax import batch_infer_morphosyntax

        nlp, ctx = english_pipeline_with_postprocessor
        req = _build_req(words)

        response = batch_infer_morphosyntax(
            req,
            nlp_pipelines={"eng": nlp},
            contexts={"eng": ctx},
            nlp_lock=threading.Lock(),
            free_threaded=False,
        )

        first_sent = _first_sentence_raw(response)
        assert first_sent, (
            f"[{label}] Empty raw_sentences for request words={words}."
        )

        # The Range entry whose parent span covers the contracted CHAT token.
        expected_chat_pos = _find_contracted_chat_position(words, head_lemma)
        range_entries = [
            w for w in first_sent
            if isinstance(w.get("id"), (list, tuple)) and len(w["id"]) == 2
        ]
        assert range_entries, (
            f"[{label}] No Range token (id=[n, n+1]) found in Preserve "
            f"payload. This is the regression: Stanza's MWT split was "
            f"merged back before reaching Rust. "
            f"Sentence: {[(w.get('id'), w.get('text')) for w in first_sent]}. "
            f"Expected Range at CHAT position {expected_chat_pos} for "
            f"{head_lemma}'s."
        )

    @pytest.mark.parametrize("label,words,head_lemma", COPULA_CONTRACTION_SENTENCES)
    def test_payload_has_component_words_for_range(
        self, english_pipeline_with_postprocessor,
        label, words, head_lemma,
    ):
        """Every Range parent must be followed by its component words.

        For a Range(n, n+1), the payload must carry two component word
        entries with Single ids n and n+1. This is what Rust's
        ``map_ud_sentence()`` consumes via ``assemble_mors()``.
        """
        import threading

        from batchalign.inference.morphosyntax import batch_infer_morphosyntax

        nlp, ctx = english_pipeline_with_postprocessor
        req = _build_req(words)

        response = batch_infer_morphosyntax(
            req,
            nlp_pipelines={"eng": nlp},
            contexts={"eng": ctx},
            nlp_lock=threading.Lock(),
            free_threaded=False,
        )

        first_sent = _first_sentence_raw(response)

        # Find any Range entry and require its components follow.
        for idx, w in enumerate(first_sent):
            wid = w.get("id")
            if isinstance(wid, (list, tuple)) and len(wid) == 2:
                start, end = wid
                # The next N = (end - start + 1) entries should be the components.
                n = end - start + 1
                following = first_sent[idx + 1 : idx + 1 + n]
                assert len(following) == n, (
                    f"[{label}] Range {wid} has no following component "
                    f"words. Got: {[(x.get('id'), x.get('text')) for x in following]}"
                )
                comp_ids = [x.get("id") for x in following]
                expected = list(range(start, end + 1))
                assert all(
                    cid == i or (isinstance(cid, (list, tuple))
                                 and len(cid) == 1 and cid[0] == i)
                    for cid, i in zip(comp_ids, expected)
                ), (
                    f"[{label}] Range {wid} component ids mismatch. "
                    f"Expected {expected}, got {comp_ids}"
                )
                return  # First Range is the one we care about for this test
        pytest.fail(
            f"[{label}] No Range token found to verify components for; "
            f"sentence: {[(w.get('id'), w.get('text')) for w in first_sent]}"
        )


# ---------------------------------------------------------------------------
# RED #2: For the two sentences where Stanza correctly tags AUX, the IPC
#         payload must carry that tag on the clitic component.
# ---------------------------------------------------------------------------


@pytest.mark.golden
class TestPreserveIPCClitcIsAUX:
    """Stanza correctly delivers AUX/be at the IPC boundary for stool/he.

    Limited to the cases Stanza gets right at the Python layer. The
    sink/lady cases are corrected downstream by the Rust
    ``nlp::invariants::finite_verb_main_clause`` rewrite — those are
    verified by end-to-end CLI tests asserting final ``%mor`` output,
    not by inspecting the IPC payload (which still reflects Stanza's
    wrong possessive reading at this layer).
    """

    @pytest.mark.parametrize(
        "label,words,head_lemma",
        [
            ("stool", ["the", "stool's", "going", "over", "."], "stool"),
            ("he", ["and", "he's", "falling", "over", "."], "he"),
        ],
    )
    def test_clitic_component_has_aux_be(
        self, english_pipeline_with_postprocessor,
        label, words, head_lemma,
    ):
        import threading

        from batchalign.inference.morphosyntax import batch_infer_morphosyntax

        nlp, ctx = english_pipeline_with_postprocessor
        response = batch_infer_morphosyntax(
            _build_req(words),
            nlp_pipelines={"eng": nlp},
            contexts={"eng": ctx},
            nlp_lock=threading.Lock(),
            free_threaded=False,
        )

        first_sent = _first_sentence_raw(response)

        # Locate the head word by lemma, then the following component.
        head_idx = None
        for idx, w in enumerate(first_sent):
            if (w.get("lemma") or "").lower() == head_lemma:
                head_idx = idx
                break
        assert head_idx is not None, (
            f"[{label}] No word with lemma {head_lemma!r} in payload. "
            f"Sentence: {[(w.get('id'), w.get('text'), w.get('lemma')) for w in first_sent]}"
        )
        clitic = first_sent[head_idx + 1] if head_idx + 1 < len(first_sent) else None
        assert clitic is not None, (
            f"[{label}] No clitic component after head {head_lemma!r}."
        )
        assert clitic.get("upos") == "AUX", (
            f"[{label}] Clitic upos should be AUX for {head_lemma}'s, "
            f"got {clitic.get('upos')!r}. Full clitic: {clitic}"
        )
        assert clitic.get("lemma") == "be", (
            f"[{label}] Clitic lemma should be 'be', got {clitic.get('lemma')!r}"
        )
