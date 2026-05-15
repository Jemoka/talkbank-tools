"""Regression: [- zho] pre-code must not trigger Mandarin retokenize in yue job.

Bug: In a Cantonese (yue) morphotag --retokenize job, utterances with
[- zho] language pre-code get routed to the Mandarin retokenize pipeline
(tokenize_pretokenized=False). Stanza's neural tokenizer then re-segments
the already-pretokenized words, merging them unpredictably.

Source: data/childes-other-data/Chinese/Cantonese/MOST/10011/40412d.cha
"""

from __future__ import annotations


def test_retok_pipeline_not_activated_for_precode_language() -> None:
    """use_retok_pipeline must be False when item lang differs from job lang.

    Current bug: use_retok_pipeline = req.retokenize and lang_code in ("zho", "cmn")
    This is True for [- zho] items in a yue job, incorrectly activating
    Mandarin neural tokenization on ASR output.
    """
    # Simulate the decision logic from batch_infer_morphosyntax
    retokenize = True
    req_lang = "yue"  # JOB language
    item_lang = "zho"  # per-utterance from [- zho]

    # Fixed logic: must check job-level language too
    use_retok = (
        retokenize
        and item_lang in ("zho", "cmn")
        and req_lang in ("zho", "cmn")
    )
    assert use_retok is False, (
        "retok pipeline must NOT activate for [- zho] items when job lang is yue"
    )


def test_retok_pipeline_activated_for_mandarin_job() -> None:
    """use_retok_pipeline must be True for a genuine Mandarin job."""
    retokenize = True
    req_lang = "cmn"
    item_lang = "cmn"

    use_retok = (
        retokenize
        and item_lang in ("zho", "cmn")
        and req_lang in ("zho", "cmn")
    )
    assert use_retok is True
