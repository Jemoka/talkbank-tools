"""Tests for the thin Python coreference inference boundary."""

from __future__ import annotations

from types import SimpleNamespace

import pytest

from batchalign.inference.coref import (
    ChainRef,
    CorefBatchItem,
    CorefRawAnnotation,
    CorefRawResponse,
    batch_infer_coref,
)
from batchalign.providers import BatchInferRequest


class TestCorefModels:
    """Verify the typed coref wire models remain stable."""

    def test_coref_batch_item_roundtrip(self) -> None:
        item = CorefBatchItem(sentences=[["the", "dog"], ["it", "ran"]])
        assert item.model_dump() == {
            "sentences": [["the", "dog"], ["it", "ran"]]
        }

    def test_coref_raw_response_roundtrip(self) -> None:
        response = CorefRawResponse(
            annotations=[
                CorefRawAnnotation(
                    sentence_idx=0,
                    words=[[ChainRef(chain_id=1, is_start=True, is_end=False)], []],
                )
            ]
        )
        data = response.model_dump()
        back = CorefRawResponse.model_validate(data)
        assert back.annotations[0].sentence_idx == 0
        assert back.annotations[0].words[0][0].chain_id == 1


def test_batch_infer_coref_reuses_pipeline_and_returns_sparse_annotations(monkeypatch) -> None:
    """One batch should initialize Stanza once and only emit sentences with chains."""

    pipeline_inits: list[dict[str, object]] = []
    seen_texts: list[str] = []

    class _FakeWord:
        def __init__(self, coref_chains: list[object] | None = None) -> None:
            self.coref_chains = coref_chains or []

    class _FakeSentence:
        def __init__(self, words: list[_FakeWord]) -> None:
            self.words = words

    class _FakePipeline:
        def __init__(self, **kwargs) -> None:
            pipeline_inits.append(kwargs)

        def __call__(self, text: str):
            seen_texts.append(text)
            if text == "the dog\n\nit ran":
                return SimpleNamespace(
                    sentences=[
                        _FakeSentence(
                            [
                                _FakeWord(
                                    [
                                        SimpleNamespace(
                                            chain=SimpleNamespace(index=7),
                                            is_start=True,
                                            is_end=True,
                                        )
                                    ]
                                ),
                                _FakeWord(),
                            ]
                        ),
                        _FakeSentence([_FakeWord(), _FakeWord()]),
                    ]
                )
            return SimpleNamespace(sentences=[_FakeSentence([_FakeWord()])])

    monkeypatch.setitem(
        __import__("sys").modules,
        "stanza",
        SimpleNamespace(Pipeline=_FakePipeline),
    )

    response = batch_infer_coref(
        BatchInferRequest(
            task="coref",
            lang="eng",
            items=[
                {"sentences": [["the", "dog"], ["it", "ran"]]},
                {"sentences": [["hello"]]},
            ],
        )
    )

    assert len(pipeline_inits) == 1
    assert pipeline_inits[0] == {
        "lang": "en",
        "processors": "tokenize, coref",
        "package": {"coref": "ontonotes-singletons_roberta-large-lora"},
        "tokenize_pretokenized": True,
    }
    assert seen_texts == ["the dog\n\nit ran", "hello"]
    assert response.results[0].error is None
    assert response.results[0].result == {
        "annotations": [
            {
                "sentence_idx": 0,
                "words": [[{"chain_id": 7, "is_start": True, "is_end": True}], []],
            }
        ]
    }
    assert response.results[1].result == {"annotations": []}


def test_batch_infer_coref_ignores_extra_sentences_from_runtime(monkeypatch) -> None:
    """Worker output with more sentences than the request should be truncated safely."""

    class _FakeWord:
        def __init__(self, coref_chains: list[object] | None = None) -> None:
            self.coref_chains = coref_chains or []

    class _FakeSentence:
        def __init__(self, words: list[_FakeWord]) -> None:
            self.words = words

    class _FakePipeline:
        def __init__(self, **_kwargs) -> None:
            pass

        def __call__(self, _text: str):
            return SimpleNamespace(
                sentences=[
                    _FakeSentence(
                        [
                            _FakeWord(
                                [
                                    SimpleNamespace(
                                        chain=SimpleNamespace(index=2),
                                        is_start=True,
                                        is_end=True,
                                    )
                                ]
                            )
                        ]
                    ),
                    _FakeSentence(
                        [
                            _FakeWord(
                                [
                                    SimpleNamespace(
                                        chain=SimpleNamespace(index=9),
                                        is_start=True,
                                        is_end=True,
                                    )
                                ]
                            )
                        ]
                    ),
                ]
            )

    monkeypatch.setitem(
        __import__("sys").modules,
        "stanza",
        SimpleNamespace(Pipeline=_FakePipeline),
    )

    response = batch_infer_coref(
        BatchInferRequest(
            task="coref",
            lang="eng",
            items=[{"sentences": [["she"]]}],
        )
    )

    assert response.results[0].result == {
        "annotations": [
            {
                "sentence_idx": 0,
                "words": [[{"chain_id": 2, "is_start": True, "is_end": True}]],
            }
        ]
    }


def test_batch_infer_coref_reports_invalid_items_and_empty_documents(monkeypatch) -> None:
    """Invalid items should fail explicitly, while empty documents stay no-op."""

    class _UnusedPipeline:
        def __init__(self, **_kwargs) -> None:
            raise AssertionError("pipeline should not be created for invalid or empty items")

    monkeypatch.setitem(
        __import__("sys").modules,
        "stanza",
        SimpleNamespace(Pipeline=_UnusedPipeline),
    )

    response = batch_infer_coref(
        BatchInferRequest(
            task="coref",
            lang="eng",
            items=[{"bad": "shape"}, {"sentences": []}],
        )
    )

    assert response.results[0].error == "Invalid CorefBatchItem"
    assert response.results[1].result == {"annotations": []}


def test_batch_infer_coref_returns_empty_annotations_on_runtime_failure(monkeypatch) -> None:
    """Unexpected Stanza failures should degrade to empty structured annotations."""

    class _ExplodingPipeline:
        def __init__(self, **_kwargs) -> None:
            pass

        def __call__(self, _text: str):
            raise RuntimeError("coref runtime exploded")

    monkeypatch.setitem(
        __import__("sys").modules,
        "stanza",
        SimpleNamespace(Pipeline=_ExplodingPipeline),
    )

    response = batch_infer_coref(
        BatchInferRequest(
            task="coref",
            lang="eng",
            items=[{"sentences": [["she"], ["left"]]}],
        )
    )

    assert response.results[0].error is None
    assert response.results[0].result == {"annotations": []}
    assert response.results[0].elapsed_s >= 0.0
