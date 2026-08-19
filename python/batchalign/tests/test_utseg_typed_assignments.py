"""Hermetic tests for typed CHATUtterance boundary assignments."""

from __future__ import annotations

from batchalign._core.proto import AsrWord, UtteranceSpan
from batchalign.backends.asr.chatwhisper import BertUtteranceModel
from batchalign.backends.utseg.chatutterance import _spans_from_assignments


def test_typed_assignments_split_only_on_sentence_boundary_actions():
    model = BertUtteranceModel.__new__(BertUtteranceModel)
    model.predict_actions = lambda _words: [0, 1, 2, 5, 3, 4]  # type: ignore[method-assign]

    assert model.predict_assignments(["a", "b", "c", "d", "e", "f"]) == [
        0,
        0,
        0,
        1,
        1,
        2,
    ]


def test_typed_actions_drop_the_earlier_adjacent_model_action():
    model = BertUtteranceModel.__new__(BertUtteranceModel)
    model._predict_word_actions = lambda _words: [2, 1, 0]  # type: ignore[method-assign]

    assert model.predict_actions(["Hello,", "THERE!", "again"]) == [0, 1, 0]


def test_typed_spans_preserve_source_case_and_words():
    words = [
        AsrWord(text="Yeah", start_ms=100, end_ms=200, confidence=0.9),
        AsrWord(text="that's", start_ms=200, end_ms=300, confidence=0.9),
        AsrWord(text="mine", start_ms=300, end_ms=400, confidence=0.9),
    ]

    spans = _spans_from_assignments(words, [0, 1, 1], UtteranceSpan)

    assert [span.text for span in spans] == ["Yeah", "that's mine"]
    assert [[word.text for word in span.words] for span in spans] == [
        ["Yeah"],
        ["that's", "mine"],
    ]
