"""Tests for the thin Python morphosyntax inference boundary."""

from __future__ import annotations

from copy import deepcopy
from types import SimpleNamespace
from typing import Any

import pytest

from batchalign.inference.morphosyntax import (
    _is_bogus_lemma,
    batch_infer_morphosyntax,
    validate_ud_words,
)
from batchalign.providers import BatchInferRequest


class _RecordingLock:
    """Minimal context-manager lock that counts acquisitions."""

    def __init__(self) -> None:
        self.enter_count = 0

    def __enter__(self) -> None:
        self.enter_count += 1

    def __exit__(self, exc_type, exc, tb) -> bool:
        return False


class _FakeDoc:
    """Tiny doc-like object exposing Stanza's ``to_dict()`` seam."""

    def __init__(self, rows: list[list[dict[str, Any]]]) -> None:
        self._rows = rows

    def to_dict(self) -> list[list[dict[str, Any]]]:
        return deepcopy(self._rows)


class _RecordingNlp:
    """Callable test double for one Stanza pipeline."""

    def __init__(
        self,
        ctx: SimpleNamespace | None,
        rows: list[list[dict[str, Any]]],
        *,
        error: Exception | None = None,
    ) -> None:
        self.ctx = ctx
        self.rows = rows
        self.error = error
        self.calls: list[tuple[str, list[list[str]]]] = []

    def __call__(self, text: str) -> _FakeDoc:
        original_words = [] if self.ctx is None else [list(words) for words in self.ctx.original_words]
        self.calls.append((text, original_words))
        if self.error is not None:
            raise self.error
        return _FakeDoc(self.rows)


def _raw_sentence(words: list[str]) -> list[dict[str, Any]]:
    """Build one minimal raw-Stanza-like sentence."""

    rows = []
    for i, word in enumerate(words, start=1):
        rows.append(
            {
                "id": i,
                "text": word,
                "lemma": word.lower(),
                "upos": "NOUN",
                "head": 0 if i == 1 else 1,
                "deprel": "root" if i == 1 else "obj",
            }
        )
    return rows


def test_is_bogus_lemma_flags_punctuation_only_lemmas() -> None:
    """Bogus-lemma detection should ignore surface matches and real empty lemmas."""

    assert _is_bogus_lemma("hello", "...") is True
    assert _is_bogus_lemma("hello", "hello") is False
    assert _is_bogus_lemma("hello", "") is False
    assert _is_bogus_lemma("?!", "?!") is False


def test_validate_ud_words_falls_back_from_bogus_punctuation_lemma() -> None:
    """Punctuation-only lemmas for lexical words should fall back to the surface form."""

    sentences = [[
        {
            "id": 1,
            "text": "bonjour",
            "lemma": "...",
            "upos": "INTJ",
            "head": 0,
            "deprel": "root",
        }
    ]]

    validate_ud_words(sentences)

    assert sentences[0][0]["lemma"] == "bonjour"
    assert sentences[0][0]["deprel"] == "root"


def test_validate_ud_words_coerces_tuple_ids_inside_sentence_rows() -> None:
    """Tuple IDs from Stanza should be normalized before row validation."""

    sentences = [[{"id": (2, 3), "text": "au"}]]

    validate_ud_words(sentences)

    assert sentences[0][0]["id"] == [2, 3]
    assert sentences[0][0]["lemma"] == ""


def test_batch_infer_morphosyntax_groups_by_language_and_uses_lock(monkeypatch) -> None:
    """Batch morphosyntax should group by language, set tokenizer context, and lock."""

    monotonic = iter([10.0, 13.5])
    monkeypatch.setattr(
        "batchalign.inference.morphosyntax.time.monotonic",
        lambda: next(monotonic),
    )

    lock = _RecordingLock()
    eng_ctx = SimpleNamespace(original_words=[])
    fra_ctx = SimpleNamespace(original_words=[])
    eng_nlp = _RecordingNlp(
        eng_ctx,
        [_raw_sentence(["hello", "world"]), _raw_sentence(["goodbye", "moon"])],
    )
    fra_nlp = _RecordingNlp(fra_ctx, [_raw_sentence(["salut"])])

    response = batch_infer_morphosyntax(
        BatchInferRequest(
            task="morphosyntax",
            lang="eng",
            items=[
                {"words": ["hello", "world"]},
                {"words": ["goodbye", "moon"]},
                {"words": ["salut"], "lang": "fra"},
                {"words": []},
                {"bad": "shape"},
            ],
        ),
        {"eng": eng_nlp, "fra": fra_nlp},
        {"eng": eng_ctx, "fra": fra_ctx},
        lock,
        free_threaded=False,
    )

    assert lock.enter_count == 2
    assert eng_nlp.calls == [
        ("hello world\n\ngoodbye moon", [["hello", "world"], ["goodbye", "moon"]])
    ]
    assert fra_nlp.calls == [("salut", [["salut"]])]
    assert eng_ctx.original_words == []
    assert fra_ctx.original_words == []
    assert response.results[0].result == {"raw_sentences": [_raw_sentence(["hello", "world"])]}
    assert response.results[0].elapsed_s == 3.5
    assert response.results[1].result == {"raw_sentences": [_raw_sentence(["goodbye", "moon"])]}
    assert response.results[2].result == {"raw_sentences": [_raw_sentence(["salut"])]}
    assert response.results[3].result == {"sentences": []}
    assert response.results[4].error == "Invalid batch item"


def test_batch_infer_morphosyntax_uses_fallback_context_and_resets_after_failure(monkeypatch) -> None:
    """If a lang-specific context is absent, the request-lang context should be used and reset."""

    monotonic = iter([1.0, 2.0])
    monkeypatch.setattr(
        "batchalign.inference.morphosyntax.time.monotonic",
        lambda: next(monotonic),
    )

    lock = _RecordingLock()
    fallback_ctx = SimpleNamespace(original_words=[])
    fra_nlp = _RecordingNlp(
        fallback_ctx,
        [],
        error=RuntimeError("stanza exploded"),
    )

    response = batch_infer_morphosyntax(
        BatchInferRequest(
            task="morphosyntax",
            lang="eng",
            items=[{"words": ["salut", "toi"], "lang": "fra"}],
        ),
        {"fra": fra_nlp},
        {"eng": fallback_ctx},
        lock,
        free_threaded=True,
    )

    assert lock.enter_count == 0
    assert fra_nlp.calls == [("salut toi", [["salut", "toi"]])]
    assert fallback_ctx.original_words == []
    assert response.results[0].result == {"sentences": []}
    assert response.results[0].elapsed_s == 1.0


def test_batch_infer_morphosyntax_leaves_defaults_for_missing_pipelines_and_mismatches(monkeypatch) -> None:
    """Missing pipelines and sentence-count drift should preserve empty fallback results."""

    monotonic = iter([20.0, 21.0])
    monkeypatch.setattr(
        "batchalign.inference.morphosyntax.time.monotonic",
        lambda: next(monotonic),
    )

    lock = _RecordingLock()
    eng_ctx = SimpleNamespace(original_words=[])
    mismatch_nlp = _RecordingNlp(
        eng_ctx,
        [_raw_sentence(["only", "one"])],
    )

    response = batch_infer_morphosyntax(
        BatchInferRequest(
            task="morphosyntax",
            lang="eng",
            items=[
                {"words": ["no", "pipeline"], "lang": "spa"},
                {"words": ["hello", "world"]},
                {"words": ["goodbye", "moon"]},
            ],
        ),
        {"eng": mismatch_nlp},
        {"eng": eng_ctx},
        lock,
        free_threaded=False,
    )

    assert mismatch_nlp.calls == [
        ("hello world\n\ngoodbye moon", [["hello", "world"], ["goodbye", "moon"]])
    ]
    assert response.results[0].result == {"sentences": []}
    assert response.results[1].result == {"sentences": []}
    assert response.results[2].result == {"sentences": []}


def test_batch_infer_morphosyntax_returns_early_when_no_nonempty_items(monkeypatch) -> None:
    """All-invalid or empty items should hit the no-work early return."""

    monotonic = iter([30.0, 31.0])
    monkeypatch.setattr(
        "batchalign.inference.morphosyntax.time.monotonic",
        lambda: next(monotonic),
    )

    response = batch_infer_morphosyntax(
        BatchInferRequest(
            task="morphosyntax",
            lang="eng",
            items=[{"words": []}, {"bad": "shape"}],
        ),
        {},
        {},
        _RecordingLock(),
        free_threaded=False,
    )

    assert response.results[0].result == {"sentences": []}
    assert response.results[0].elapsed_s == 0.0
    assert response.results[1].error == "Invalid batch item"
