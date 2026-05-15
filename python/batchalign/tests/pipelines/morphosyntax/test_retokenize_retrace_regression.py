"""Regression test: retokenize must not fail on utterances with retraces.

Bug: morphotag --retokenize on MOST corpus file 40415b.cha failed with
"MOR item count (5) does not match alignable word count (6)" on utterance:
*PAR0: 呢 度 <下次> [/] 食飯 啦 飯 啦 .

The retrace <下次> [/] is skipped during MOR extraction, leaving 6
alignable words. After _segment_cantonese (which preserves existing
multi-char tokens), Stanza gets 6 words and returns 6 MOR items.
The Rust injection step should accept 6 MOR items for 6 words.

This test verifies the Python side returns the correct count.
The Rust side is tested separately.

Source: data/childes-other-data/Chinese/Cantonese/MOST/10002/40415b.cha
"""

from __future__ import annotations

import pytest


@pytest.mark.golden
def test_retokenize_retrace_utterance_returns_correct_count() -> None:
    """Stanza returns 6 MOR items for 6 words in the retrace utterance.

    The words are: 呢 度 食飯 啦 飯 啦 (retrace <下次> [/] already removed
    by Rust before reaching Python).
    """
    import stanza
    import threading
    from stanza import DownloadMethod
    from batchalign.inference.morphosyntax import batch_infer_morphosyntax
    from batchalign.inference._tokenizer_realign import TokenizerContext
    from batchalign.worker._types import BatchInferRequest, InferTask

    nlp = stanza.Pipeline(
        lang="zh",
        processors="tokenize,pos,lemma,depparse",
        download_method=DownloadMethod.REUSE_RESOURCES,
        tokenize_no_ssplit=True,
        tokenize_pretokenized=True,
    )

    words = ["呢", "度", "食飯", "啦", "飯", "啦"]
    req = BatchInferRequest(
        task=InferTask.MORPHOSYNTAX,
        lang="yue",
        items=[{"words": words, "terminator": ".", "lang": "yue"}],
        retokenize=True,
    )

    resp = batch_infer_morphosyntax(
        req=req,
        nlp_pipelines={"yue": nlp},
        contexts={"yue": TokenizerContext()},
        nlp_lock=threading.Lock(),
        free_threaded=False,
    )

    result = resp.results[0]
    assert result.error is None, f"Inference failed: {result.error}"

    sentences = result.result["raw_sentences"]
    assert len(sentences) == 1, f"Expected 1 sentence, got {len(sentences)}"

    ud_words = sentences[0]
    assert len(ud_words) == 6, (
        f"Stanza should return 6 UD words for 6 input tokens, got {len(ud_words)}: "
        f"{[w.get('text') for w in ud_words]}"
    )
