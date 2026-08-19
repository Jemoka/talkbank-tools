"""Hermetic tests for Rev's pre-CHAT utterance segmentation."""

from __future__ import annotations

from batchalign._core.proto import AsrSegment, AsrWord
from batchalign.backends.asr.rev import _presegment_raw_segments


def test_rev_splits_raw_words_from_typed_assignments_before_chat_cleanup():
    class Segmenter:
        @staticmethod
        def predict_assignments(words):
            assert words == ["uh", "yeah", "that's", "mine"]
            return [0, 0, 1, 1]

    segment = AsrSegment(
        start_ms=100,
        end_ms=500,
        text="uh yeah that's mine",
        speaker="1",
        words=[
            AsrWord(text="uh", start_ms=100, end_ms=200, confidence=0.9),
            AsrWord(text="yeah", start_ms=200, end_ms=300, confidence=0.9),
            AsrWord(text="that's", start_ms=300, end_ms=400, confidence=0.9),
            AsrWord(text="mine", start_ms=400, end_ms=500, confidence=0.9),
        ],
    )

    output = _presegment_raw_segments([segment], Segmenter(), AsrSegment)

    assert [item.text for item in output] == ["uh yeah", "that's mine"]
    assert [(item.start_ms, item.end_ms) for item in output] == [
        (100, 300),
        (300, 500),
    ]
    assert [item.speaker for item in output] == ["1", "1"]
