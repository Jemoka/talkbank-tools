"""Tests for retokenize MWT (Multi-Word Token) contraction expansion.

These tests verify that English contractions like 'gonna', 'don't', 'it's'
are properly expanded when retokenize is requested, matching BA2-Jan9 behavior.

The BA2 reference shows:
- gonna → verb|go-Part-Pres-S~part|to  (Range token, clitic MOR)
- don't → aux|do-Fin-Ind-Pres-S1~part|not

BA3 regression: pretokenized mode + postprocessor merges tokens back,
preventing MWT expansion even when retokenize=True.

Each test isolates one question about the tokenization/MWT pipeline.
"""


import pytest

# Stanza pipeline fixtures are provided by conftest.py in this directory.


# ---------------------------------------------------------------------------
# Q1: Does Stanza's free tokenizer expand 'gonna'?
# ---------------------------------------------------------------------------


@pytest.mark.golden
class TestStanzaFreeTokenizeExpandsContractions:
    """Stanza's free tokenizer produces MWT Range tokens for contractions."""

    def test_gonna_produces_range_token(self, english_pipeline_free_tokenize):
        doc = english_pipeline_free_tokenize("gonna eat cookies .")
        first_token = doc.sentences[0].tokens[0]
        assert first_token.id == (1, 2), (
            f"'gonna' should be Range token (1,2), got id={first_token.id}"
        )
        assert [w.text for w in first_token.words] == ["gon", "na"]

    def test_dont_produces_range_token(self, english_pipeline_free_tokenize):
        doc = english_pipeline_free_tokenize("I don't know .")
        dont_token = doc.sentences[0].tokens[1]
        assert dont_token.id == (2, 3), (
            f"'don't' should be Range token (2,3), got id={dont_token.id}"
        )
        assert [w.text for w in dont_token.words] == ["do", "n't"]

    def test_its_produces_range_token(self, english_pipeline_free_tokenize):
        doc = english_pipeline_free_tokenize("it's working .")
        first_token = doc.sentences[0].tokens[0]
        assert first_token.id == (1, 2), (
            f"'it's' should be Range token (1,2), got id={first_token.id}"
        )
        assert [w.text for w in first_token.words] == ["it", "'s"]


# ---------------------------------------------------------------------------
# Q2: Does pretokenized mode suppress MWT expansion?
# ---------------------------------------------------------------------------


@pytest.mark.golden
class TestPretokenizedSuppressesMWT:
    """Pretokenized mode does NOT expand contractions."""

    def test_gonna_stays_single_token(self, english_pipeline_pretokenized):
        doc = english_pipeline_pretokenized([["gonna", "eat", "cookies", "."]])
        first_token = doc.sentences[0].tokens[0]
        assert first_token.id == (1,), (
            f"Pretokenized 'gonna' should be Single (1,), got id={first_token.id}"
        )
        assert first_token.text == "gonna"

    def test_dont_stays_single_token(self, english_pipeline_pretokenized):
        doc = english_pipeline_pretokenized([["I", "don't", "know", "."]])
        dont_token = doc.sentences[0].tokens[1]
        assert dont_token.id == (2,)
        assert dont_token.text == "don't"


# ---------------------------------------------------------------------------
# Q3: With the postprocessor and original_words SET (keeptokens), are
#     contractions merged back?
# ---------------------------------------------------------------------------


@pytest.mark.golden
class TestPostprocessorWithOriginalWordsMerges:
    """Postprocessor with original_words set preserves Stanza MWT hints.

    History: before the 2026-04-13 fix to ``_realign_sentence``, this
    class asserted that Stanza's MWT splits were merged back to Single
    tokens — the Preserve-mode MWT regression. After the fix,
    ``_realign_sentence`` preserves Stanza's native ``(text, True)``
    hints, so MWT expansion survives and the first entry is a Range.
    """

    def test_gonna_preserved_as_range(self, english_pipeline_with_postprocessor):
        nlp, ctx = english_pipeline_with_postprocessor
        ctx.original_words = [["gonna", "eat", "cookies", "."]]
        doc = nlp("gonna eat cookies .")
        ctx.original_words = []

        sents = doc.to_dict()
        first_word = sents[0][0]
        # After the MWT hint fix, gonna survives as a Range (1,2) Token
        # with component words "gon" + "na".
        assert first_word["text"] == "gonna", (
            f"Range parent text should be 'gonna', got: {first_word['text']}"
        )
        word_id = first_word["id"]
        assert word_id == [1, 2] or word_id == (1, 2), (
            f"Should be Range id [1, 2] (MWT preserved), got: {word_id}. "
            f"If this fails, _realign_sentence is stripping Stanza hints again."
        )


# ---------------------------------------------------------------------------
# Q4: With the postprocessor and original_words NOT SET (retokenize mode),
#     do contractions pass through as Range tokens?
# ---------------------------------------------------------------------------


@pytest.mark.golden
class TestPostprocessorWithoutOriginalWordsPassesThrough:
    """Postprocessor without original_words lets MWT expansion through."""

    def test_gonna_expanded_to_range(self, english_pipeline_with_postprocessor):
        nlp, ctx = english_pipeline_with_postprocessor
        # Do NOT set original_words (simulates retokenize mode)
        doc = nlp("gonna eat cookies .")

        sents = doc.to_dict()
        first_word = sents[0][0]
        word_id = first_word.get("id")
        assert word_id == [1, 2] or word_id == (1, 2), (
            f"Without original_words, 'gonna' should be Range (1,2), "
            f"got id={word_id}, text={first_word.get('text')}"
        )

    def test_dont_expanded_to_range(self, english_pipeline_with_postprocessor):
        nlp, ctx = english_pipeline_with_postprocessor
        doc = nlp("I don't know .")

        sents = doc.to_dict()
        # First word should be "I", second should be Range for "don't"
        dont_entry = sents[0][1]
        dont_id = dont_entry.get("id")
        assert dont_id == [2, 3] or dont_id == (2, 3), (
            f"Without original_words, 'don't' should be Range (2,3), "
            f"got id={dont_id}, text={dont_entry.get('text')}"
        )


# ---------------------------------------------------------------------------
# Q5: Does batch_infer_morphosyntax pass retokenize=True to the pipeline
#     correctly (i.e., NOT set original_words)?
# ---------------------------------------------------------------------------


@pytest.mark.golden
class TestBatchInferRetokenize:
    """batch_infer_morphosyntax with retokenize=True should expand contractions."""

    def test_gonna_expanded_in_batch_infer(self, english_pipeline_with_postprocessor):
        from batchalign.inference.morphosyntax import batch_infer_morphosyntax
        from batchalign.worker._types import BatchInferRequest

        nlp, ctx = english_pipeline_with_postprocessor

        req = BatchInferRequest(
            task="morphosyntax",
            items=[{
                "words": ["gonna", "eat", "cookies", "."],
                "terminator": ".",
                "special_forms": [[None, None]] * 4,
                "lang": "eng",
            }],
            lang="eng",
            retokenize=True,
            mwt={},
        )

        import threading

        response = batch_infer_morphosyntax(
            req,
            nlp_pipelines={"eng": nlp},
            contexts={"eng": ctx},
            nlp_lock=threading.Lock(),
            free_threaded=False,
        )

        result = response.results[0].result
        raw = result.get("raw_sentences", [[]])
        first_sent = raw[0] if raw else []
        first_word = first_sent[0] if first_sent else {}

        word_id = first_word.get("id")
        assert word_id == [1, 2] or word_id == (1, 2), (
            f"batch_infer with retokenize=True should expand 'gonna' to Range, "
            f"got id={word_id}, text={first_word.get('text')}"
        )


# ---------------------------------------------------------------------------
# Q5b: Does the real worker pipeline (loaded via load_stanza_models) produce
#      Range tokens for English MWT contractions with retokenize=True?
#
# Q5 used a manually-constructed pipeline with the correct MWT config.
# Q5b verifies the ACTUAL worker loading path produces the same result.
# Historical bug: ``MWT_LANGS`` once omitted ``"en"``, so
# load_stanza_models("eng") took the ``not has_mwt`` branch with
# tokenize_pretokenized=True, making the English-specific MWT+postprocessor
# branch dead code. The hardcoded list was retired on 2026-04-15 in favor
# of a runtime read from Stanza's resources.json (``should_request_mwt``).
# ---------------------------------------------------------------------------


@pytest.mark.golden
class TestWorkerPipelineRetokenize:
    """The real worker pipeline must produce Range tokens for retokenize=True."""

    def test_worker_pipeline_gonna_expanded(self):
        """load_stanza_models('eng') pipeline with retokenize=True expands 'gonna'."""
        import threading

        from batchalign.inference.morphosyntax import batch_infer_morphosyntax
        from batchalign.worker._stanza_loading import load_stanza_models
        from batchalign.worker._types import BatchInferRequest, _state

        load_stanza_models("eng")
        assert _state.stanza_pipelines is not None

        req = BatchInferRequest(
            task="morphosyntax",
            items=[{
                "words": ["gonna", "eat", "cookies", "."],
                "terminator": ".",
                "special_forms": [[None, None]] * 4,
                "lang": "eng",
            }],
            lang="eng",
            retokenize=True,
            mwt={},
        )

        response = batch_infer_morphosyntax(
            req,
            nlp_pipelines=_state.stanza_pipelines,
            contexts=_state.stanza_contexts or {},
            nlp_lock=threading.Lock(),
            free_threaded=False,
        )

        result = response.results[0].result
        raw = result.get("raw_sentences", [[]])
        first_sent = raw[0] if raw else []
        first_word = first_sent[0] if first_sent else {}

        word_id = first_word.get("id")
        assert word_id == [1, 2] or word_id == (1, 2), (
            f"Worker pipeline with retokenize=True should expand 'gonna' to "
            f"Range token, got id={word_id}, text={first_word.get('text', '?')}. "
            f"Check that should_request_mwt('en', table) is True in "
            f"_stanza_loading.py — the capability table must report "
            f"has_mwt=True for English."
        )


# ---------------------------------------------------------------------------
# Q6: Does the V2 execute handler preserve retokenize=True through the full
#     path (artifact loading -> batch_infer -> normalization -> response)?
#
# Q5 tests batch_infer_morphosyntax directly. Q6 tests the V2 execute handler
# which wraps batch_infer with artifact loading and result normalization via
# batchalign_core.normalize_text_task_result(). The normalization is a
# potential lossy boundary where Range token data could be dropped.
# ---------------------------------------------------------------------------


def _run_v2_morphosyntax(nlp, ctx, tmp_path, *, retokenize, request_id):
    """Build host + artifact + request, execute, return first token id."""
    import json
    import threading

    from batchalign.inference.morphosyntax import batch_infer_morphosyntax
    from batchalign.worker._text_v2 import (
        TextExecutionHostV2,
        execute_morphosyntax_request_v2,
    )
    from batchalign.worker._types import BatchInferRequest, BatchInferResponse
    from batchalign.worker._types_v2 import (
        ExecuteRequestV2,
        InferenceTaskV2,
        MorphosyntaxRequestV2,
        PreparedTextEncodingV2,
        PreparedTextRefV2,
    )

    nlp_lock = threading.Lock()

    def _runner(req: BatchInferRequest) -> BatchInferResponse:
        return batch_infer_morphosyntax(
            req=req,
            nlp_pipelines={"eng": nlp},
            contexts={"eng": ctx},
            nlp_lock=nlp_lock,
            free_threaded=False,
        )

    host = TextExecutionHostV2(morphosyntax_runner=_runner)

    batch_payload = {
        "items": [
            {
                "words": ["gonna", "eat", "cookies", "."],
                "terminator": ".",
                "special_forms": [[None, None]] * 4,
                "lang": "eng",
            }
        ],
        "mwt": {},
    }
    artifact_path = tmp_path / f"morphosyntax_batch_{request_id}.json"
    raw_json = json.dumps(batch_payload).encode("utf-8")
    artifact_path.write_bytes(raw_json)

    payload_ref_id = f"test-batch-payload-{request_id}"
    request = ExecuteRequestV2(
        request_id=request_id,
        task=InferenceTaskV2.MORPHOSYNTAX,
        payload=MorphosyntaxRequestV2(
            lang="eng",
            payload_ref_id=payload_ref_id,
            item_count=1,
            retokenize=retokenize,
        ),
        attachments=[
            PreparedTextRefV2(
                id=payload_ref_id,
                path=str(artifact_path),
                encoding=PreparedTextEncodingV2.UTF8_JSON,
                byte_offset=0,
                byte_len=len(raw_json),
            ),
        ],
    )

    response = execute_morphosyntax_request_v2(request, host)

    assert response.outcome.kind == "success", f"V2 execute failed: {response.outcome}"
    assert response.result is not None, "V2 response should have a result payload"
    items = response.result.items
    assert len(items) == 1
    assert items[0].error is None, f"Item error: {items[0].error}"
    assert items[0].raw_sentences is not None

    first_sent = items[0].raw_sentences[0]
    first_token = first_sent[0]
    return first_token.get("id") if isinstance(first_token, dict) else None


@pytest.mark.golden
class TestV2ExecuteHandlerRetokenize:
    """V2 execute handler with retokenize=True preserves MWT expansion."""

    def test_gonna_range_token_survives_v2_handler(
        self, english_pipeline_with_postprocessor, tmp_path,
    ):
        nlp, ctx = english_pipeline_with_postprocessor
        token_id = _run_v2_morphosyntax(
            nlp, ctx, tmp_path, retokenize=True, request_id="retok-v2",
        )
        assert token_id == [1, 2], (
            f"V2 handler with retokenize=True should preserve Range token "
            f"for 'gonna', got id={token_id}"
        )

    def test_retokenize_false_preserves_mwt_range(
        self, english_pipeline_with_postprocessor, tmp_path,
    ):
        nlp, ctx = english_pipeline_with_postprocessor
        token_id = _run_v2_morphosyntax(
            nlp, ctx, tmp_path, retokenize=False, request_id="no-retok-v2",
        )
        # With retokenize=False (Preserve mode), the postprocessor now
        # preserves Stanza's native MWT hint so Range tokens survive to
        # the IPC boundary. Rust's map_ud_sentence() then tilde-joins.
        # History: prior to the 2026-04-13 fix, this test asserted the
        # inverse (single-id 1) — the Preserve-mode MWT regression.
        assert token_id == [1, 2], (
            f"V2 handler with retokenize=False should produce Range [1, 2] "
            f"for 'gonna' (MWT preserved), got id={token_id}. If this fails, "
            f"the Preserve-mode MWT regression has returned."
        )
