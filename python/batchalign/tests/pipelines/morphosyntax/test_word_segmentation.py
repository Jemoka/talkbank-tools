"""Tests for PyCantonese word segmentation and retokenize wire plumbing."""

from __future__ import annotations


# ---------------------------------------------------------------------------
# Wire type tests
# ---------------------------------------------------------------------------


def test_morphosyntax_request_v2_accepts_retokenize():
    """MorphosyntaxRequestV2 must accept and round-trip the retokenize field."""
    from batchalign.worker._types_v2 import MorphosyntaxRequestV2

    req = MorphosyntaxRequestV2(
        kind="morphosyntax", lang="yue", payload_ref_id="p1", item_count=1, retokenize=True
    )
    assert req.retokenize is True


def test_morphosyntax_request_v2_retokenize_defaults_false():
    """Backward compat: retokenize defaults to False."""
    from batchalign.worker._types_v2 import MorphosyntaxRequestV2

    req = MorphosyntaxRequestV2(
        kind="morphosyntax", lang="eng", payload_ref_id="p1", item_count=1
    )
    assert req.retokenize is False


def test_batch_infer_request_accepts_retokenize():
    """BatchInferRequest must accept the retokenize field."""
    from batchalign.worker._types import BatchInferRequest, InferTask

    req = BatchInferRequest(task=InferTask.MORPHOSYNTAX, lang="eng", items=[], retokenize=True)
    assert req.retokenize is True


# ---------------------------------------------------------------------------
# PyCantonese segmentation tests
# ---------------------------------------------------------------------------


def test_segment_cantonese_basic():
    """Characters should be grouped into words by PyCantonese."""
    from batchalign.inference.morphosyntax import _segment_cantonese

    result = _segment_cantonese(["故", "事", "係", "好"])
    # PyCantonese should group some characters into words
    assert len(result) <= 4  # At minimum shouldn't get more than input
    assert "".join(result) == "故事係好"  # All characters preserved


def test_segment_cantonese_empty():
    """Empty input returns empty output."""
    from batchalign.inference.morphosyntax import _segment_cantonese

    assert _segment_cantonese([]) == []


def test_segment_cantonese_single_char():
    """Single character input passes through."""
    from batchalign.inference.morphosyntax import _segment_cantonese

    result = _segment_cantonese(["好"])
    assert result == ["好"]


def test_segment_cantonese_preserves_existing_multichar():
    """Multi-character tokens in input must NOT be re-segmented across boundaries.

    Regression test: MOST corpus utterance with retrace had existing multi-char
    words (食飯) mixed with single-char words (啦). Naive join-and-resegment
    merged 啦+飯+啦 into one token, breaking word alignment.

    Source: data/childes-other-data/Chinese/Cantonese/MOST/10002/40415b.cha
    Utterance: *PAR0: 呢 度 <下次> [/] 食飯 啦 飯 啦 .
    """
    from batchalign.inference.morphosyntax import _segment_cantonese

    # Input after MOR extraction (retrace skipped): existing multi-char 食飯
    words = ["呢", "度", "食飯", "啦", "飯", "啦"]
    result = _segment_cantonese(words)

    # 食飯 must remain as one token — not merged with neighbors
    assert "食飯" in result, (
        f"食飯 should be preserved as one token, got {result}"
    )
    # 啦飯啦 must NOT appear — that's the bug
    assert "啦飯啦" not in result, (
        f"啦飯啦 should not exist — words were wrongly merged: {result}"
    )
    # All characters preserved
    assert "".join(result) == "".join(words), (
        f"All characters must be preserved: {''.join(result)} != {''.join(words)}"
    )


def test_segment_cantonese_mixed_single_and_multi():
    """Mixed per-char and multi-char input: only per-char runs get segmented.

    Input: ["我", "想", "去", "買", "故事", "書"]
    故事 is already a multi-char word — should be preserved.
    Per-char tokens 我想去買 and 書 may be re-segmented.
    """
    from batchalign.inference.morphosyntax import _segment_cantonese

    words = ["我", "想", "去", "買", "故事", "書"]
    result = _segment_cantonese(words)

    assert "故事" in result, f"故事 should be preserved, got {result}"
    assert "".join(result) == "".join(words), "All characters preserved"
