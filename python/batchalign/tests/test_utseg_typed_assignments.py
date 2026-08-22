"""Hermetic tests for typed CHATUtterance boundary assignments."""

from __future__ import annotations

from types import SimpleNamespace

from batchalign._core.proto import (
    AsrSegment,
    AsrWord,
    LanguageSpecPerFile,
    UtSegInput,
    UtteranceSpan,
)
from batchalign.backends.asr.chatwhisper import BertUtteranceModel
from batchalign.backends.utseg.chatutterance import (
    CHATUtteranceBackend,
    _spans_from_assignments,
)


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


def test_typed_model_batches_independent_sequences_into_shared_forwards():
    import torch

    class Encoding:
        def __init__(self, token_ids):
            self.input_ids = torch.tensor([token_ids], dtype=torch.int64)
            self._word_ids = list(range(len(token_ids)))

        def word_ids(self, _batch_index):
            return self._word_ids

    class Tokenizer:
        cls_token_id = 101
        sep_token_id = 102
        pad_token_id = 0

        def __call__(self, sequences, **_kwargs):
            token_ids = [2 if word == "split" else 6 for word in sequences[0]]
            return Encoding(token_ids)

    class Model:
        config = SimpleNamespace(max_position_embeddings=512, num_labels=6)

        def __init__(self):
            self.batch_sizes = []

        def __call__(self, *, input_ids, attention_mask):
            del attention_mask
            self.batch_sizes.append(int(input_ids.shape[0]))
            actions = (input_ids == 2).to(torch.int64) * 2
            logits = torch.zeros((*input_ids.shape, 6), dtype=torch.float32)
            logits.scatter_(2, actions.unsqueeze(-1), 1.0)
            return SimpleNamespace(logits=logits)

    model = BertUtteranceModel.__new__(BertUtteranceModel)
    model.tokenizer = Tokenizer()
    model.model = Model()
    model.device = torch.device("cpu")

    sequences = [["keep", "split", "tail"] for _ in range(40)]
    assignments = model.predict_assignments_batch(sequences)

    assert assignments == [[0, 0, 1] for _ in sequences]
    assert model.model.batch_sizes == [32, 8]


def test_chatutterance_backend_batches_typed_segments():
    class Segmenter:
        def __init__(self):
            self.calls = []

        def predict_assignments_batch(self, sequences):
            self.calls.append(sequences)
            return [[0, 1] for _ in sequences]

        def predict_assignments(self, _words):
            raise AssertionError("serial predictor should not be called")

    segmenter = Segmenter()
    backend = CHATUtteranceBackend.__new__(CHATUtteranceBackend)
    backend._lang = "eng"
    backend._cantonese = False
    backend._cleanup = {}
    backend._segmenter = segmenter
    segments = []
    for offset, words in enumerate((["One", "two"], ["Three", "four"])):
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

    [output] = backend.call(
        [
            UtSegInput(
                source_id="fixture.wav",
                segments=segments,
                language=LanguageSpecPerFile(kind="per_file"),
                stanza_fallback=False,
            )
        ]
    )

    assert segmenter.calls == [[["One", "two"], ["Three", "four"]]]
    assert [span.text for span in output.utterances] == [
        "One",
        "two",
        "Three",
        "four",
    ]
