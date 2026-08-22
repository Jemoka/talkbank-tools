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


def test_rev_batches_raw_monologue_assignments():
    class Segmenter:
        def __init__(self):
            self.calls = []

        def predict_assignments_batch(self, sequences):
            self.calls.append(sequences)
            return [[0, 1] for _ in sequences]

        def predict_assignments(self, _words):
            raise AssertionError("serial predictor should not be called")

    segments = []
    for offset, words in enumerate((["one", "two"], ["three", "four"])):
        timed_words = [
            AsrWord(
                text=word,
                start_ms=offset * 1000 + index * 100,
                end_ms=offset * 1000 + (index + 1) * 100,
                confidence=0.9,
            )
            for index, word in enumerate(words)
        ]
        segments.append(
            AsrSegment(
                start_ms=timed_words[0].start_ms,
                end_ms=timed_words[-1].end_ms,
                text=" ".join(words),
                speaker="1",
                words=timed_words,
            )
        )

    segmenter = Segmenter()
    output = _presegment_raw_segments(segments, segmenter, AsrSegment)

    assert segmenter.calls == [[["one", "two"], ["three", "four"]]]
    assert [item.text for item in output] == ["one", "two", "three", "four"]
