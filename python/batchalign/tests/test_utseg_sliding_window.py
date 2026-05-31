"""Tests for `chunk_words_for_bert` sliding-window helper.

The classifier in `BertUtteranceModel.__call__` calls the pure helper to
slice long passages; this guards the chunking logic in isolation (no
torch / transformers / BERT model required).
"""

from __future__ import annotations

import pytest

from batchalign.backends.asr.chatwhisper import chunk_words_for_bert


def test_short_passage_single_chunk() -> None:
    words = ["a", "b", "c"]
    chunks = chunk_words_for_bert(words, chunk_size=400, overlap=32)
    assert chunks == [(0, ["a", "b", "c"])]


def test_exactly_chunk_size_single_chunk() -> None:
    words = [f"w{i}" for i in range(400)]
    chunks = chunk_words_for_bert(words, chunk_size=400, overlap=32)
    assert len(chunks) == 1
    assert chunks[0][0] == 0
    assert chunks[0][1] == words


def test_long_passage_overlapping_chunks() -> None:
    words = [f"w{i}" for i in range(1000)]
    chunks = chunk_words_for_bert(words, chunk_size=400, overlap=32)
    # First chunk: [0, 400). Step = 400 - 32 = 368.
    assert chunks[0][0] == 0
    assert len(chunks[0][1]) == 400
    assert chunks[1][0] == 368
    assert len(chunks[1][1]) == 400
    # Final chunk covers up to len(words).
    assert chunks[-1][0] + len(chunks[-1][1]) == 1000


def test_chunks_cover_every_index() -> None:
    words = [f"w{i}" for i in range(2500)]
    chunks = chunk_words_for_bert(words, chunk_size=400, overlap=32)
    covered: set[int] = set()
    for start, chunk in chunks:
        for j in range(len(chunk)):
            covered.add(start + j)
    assert covered == set(range(2500))


def test_invalid_chunk_size_rejected() -> None:
    with pytest.raises(ValueError):
        chunk_words_for_bert(["a"], chunk_size=0, overlap=0)


def test_invalid_overlap_rejected() -> None:
    with pytest.raises(ValueError):
        chunk_words_for_bert(["a"], chunk_size=10, overlap=10)
    with pytest.raises(ValueError):
        chunk_words_for_bert(["a"], chunk_size=10, overlap=-1)
