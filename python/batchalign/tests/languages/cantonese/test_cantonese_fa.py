# affects: python/batchalign/inference/languages/cantonese/_cantonese_fa.py
"""Unit tests for _cantonese_fa.py — jyutping conversion and FA provider."""

from __future__ import annotations

import builtins
import sys

import pytest

from batchalign.inference.languages.cantonese._cantonese_fa import (
    CantoneseFaHost,
    _hanzi_to_jyutping,
    _maybe_romanize,
    default_cantonese_fa_host,
    infer_cantonese_fa,
    load_cantonese_fa,
)
from batchalign.worker._types import BatchInferRequest, InferTask

from .conftest import PyCantoneseFake  # still used by FA host fixture in conftest


# ---------------------------------------------------------------------------
# _hanzi_to_jyutping
# ---------------------------------------------------------------------------


class TestHanziToJyutping:
    """Tests use real PyCantonese — no faked jyutping dictionary."""

    def test_single_char(self, pc_real) -> None:  # type: ignore[no-untyped-def]
        assert _hanzi_to_jyutping(pc_real, "好") == "hou"

    def test_multi_char(self, pc_real) -> None:  # type: ignore[no-untyped-def]
        assert _hanzi_to_jyutping(pc_real, "你好") == "nei'hou"

    def test_unknown_char_passthrough(self, pc_real) -> None:  # type: ignore[no-untyped-def]
        assert _hanzi_to_jyutping(pc_real, "xyz") == "xyz"

    def test_mixed_known_unknown(self, pc_real) -> None:  # type: ignore[no-untyped-def]
        assert _hanzi_to_jyutping(pc_real, "好x") == "hou"

    def test_empty_string(self, pc_real) -> None:  # type: ignore[no-untyped-def]
        assert _hanzi_to_jyutping(pc_real, "") == ""

    def test_corpus_word_gam(self, pc_real) -> None:  # type: ignore[no-untyped-def]
        assert _hanzi_to_jyutping(pc_real, "咁") == "gam"

    def test_corpus_word_gaau(self, pc_real) -> None:  # type: ignore[no-untyped-def]
        assert _hanzi_to_jyutping(pc_real, "搞") == "gaau"

    def test_tone_stripping(self, pc_real) -> None:  # type: ignore[no-untyped-def]
        assert _hanzi_to_jyutping(pc_real, "我") == "ngo"
        assert _hanzi_to_jyutping(pc_real, "係") == "hai"

    def test_type_error_returns_original_text(self) -> None:
        class _BrokenPc:
            @staticmethod
            def characters_to_jyutping(_text: str):
                return None

        assert _hanzi_to_jyutping(_BrokenPc(), "你好") == "你好"


# ---------------------------------------------------------------------------
# _maybe_romanize
# ---------------------------------------------------------------------------


class TestMaybeRomanize:
    """Tests use real PyCantonese — no faked jyutping dictionary."""

    def test_yue_romanizes(self, pc_real) -> None:  # type: ignore[no-untyped-def]
        assert _maybe_romanize(pc_real, "好", "yue") == "hou"

    def test_non_yue_passthrough(self, pc_real) -> None:  # type: ignore[no-untyped-def]
        assert _maybe_romanize(pc_real, "hello", "eng") == "hello"

    def test_corpus_phrase_words(self, pc_real) -> None:  # type: ignore[no-untyped-def]
        words = ["咁", "搞", "笑"]
        romanized = [_maybe_romanize(pc_real, w, "yue") for w in words]
        assert romanized == ["gam", "gaau", "siu"]


# ---------------------------------------------------------------------------
# infer_cantonese_fa (with faked model environment)
# ---------------------------------------------------------------------------


class TestInferCantoneseFa:
    def test_empty_batch(self, cantonese_fa_host: CantoneseFaHost) -> None:
        req = BatchInferRequest(task=InferTask.FA, lang="yue", items=[])
        resp = infer_cantonese_fa(req, host=cantonese_fa_host)
        assert len(resp.results) == 0

    def test_single_item(self, cantonese_fa_host: CantoneseFaHost) -> None:
        item = {
            "words": ["你", "好"],
            "word_ids": ["w1", "w2"],
            "word_utterance_indices": [0, 0],
            "word_utterance_word_indices": [0, 1],
            "audio_path": "test.wav",
            "audio_start_ms": 0,
            "audio_end_ms": 1000,
        }
        req = BatchInferRequest(task=InferTask.FA, lang="yue", items=[item])
        resp = infer_cantonese_fa(req, host=cantonese_fa_host)
        assert len(resp.results) == 1
        assert resp.results[0].error is None
        assert resp.results[0].result is not None

    def test_empty_words_shortcut(self, cantonese_fa_host: CantoneseFaHost) -> None:
        item = {
            "words": [],
            "word_ids": [],
            "word_utterance_indices": [],
            "word_utterance_word_indices": [],
            "audio_path": "test.wav",
            "audio_start_ms": 0,
            "audio_end_ms": 1000,
        }
        req = BatchInferRequest(task=InferTask.FA, lang="yue", items=[item])
        resp = infer_cantonese_fa(req, host=cantonese_fa_host)
        assert len(resp.results) == 1
        assert resp.results[0].error is None
        assert resp.results[0].result["indexed_timings"] == []

    def test_returns_error_when_model_not_loaded(self, monkeypatch) -> None:
        import batchalign.inference.languages.cantonese._cantonese_fa as module

        monkeypatch.setattr(module, "_model", None)
        monkeypatch.setattr(module, "_pc", None)
        req = BatchInferRequest(
            task=InferTask.FA,
            lang="yue",
            items=[
                {
                    "words": ["你"],
                    "word_ids": ["w1"],
                    "word_utterance_indices": [0],
                    "word_utterance_word_indices": [0],
                    "audio_path": "test.wav",
                    "audio_start_ms": 0,
                    "audio_end_ms": 1000,
                }
            ],
        )

        resp = infer_cantonese_fa(req)

        assert resp.results[0].error == (
            "Cantonese FA model not loaded — call load_cantonese_fa first"
        )

    def test_marks_invalid_items(self, cantonese_fa_host: CantoneseFaHost) -> None:
        req = BatchInferRequest(task=InferTask.FA, lang="yue", items=[{"words": ["你"]}])

        resp = infer_cantonese_fa(req, host=cantonese_fa_host)

        assert resp.results[0].error == "Invalid FaInferItem"

    def test_runtime_failures_fall_back_to_empty_timings(self) -> None:
        class _FakeAudioFile:
            def chunk(self, _start_ms: int, _end_ms: int):
                return object()

        def boom(*_args, **_kwargs):
            raise RuntimeError("alignment failed")

        host = CantoneseFaHost(
            model=object(),
            romanizer=PyCantoneseFake(),
            load_audio_file=lambda _path: _FakeAudioFile(),
            infer_wave2vec_fa=boom,
        )
        item = {
            "words": ["你", "好"],
            "word_ids": ["w1", "w2"],
            "word_utterance_indices": [0, 0],
            "word_utterance_word_indices": [0, 1],
            "audio_path": "test.wav",
            "audio_start_ms": 0,
            "audio_end_ms": 1000,
        }

        resp = infer_cantonese_fa(
            BatchInferRequest(task=InferTask.FA, lang="yue", items=[item]),
            host=host,
        )

        assert resp.results[0].error is None
        assert resp.results[0].result["indexed_timings"] == []


def test_load_cantonese_fa_reports_missing_dependency(monkeypatch) -> None:
    original_import = builtins.__import__

    def fake_import(name, *args, **kwargs):
        if name == "pycantonese":
            raise ImportError("missing")
        return original_import(name, *args, **kwargs)

    monkeypatch.setattr(builtins, "__import__", fake_import)

    with pytest.raises(ImportError, match="pycantonese"):
        load_cantonese_fa("yue", None)


def test_default_cantonese_fa_host_requires_loaded_state(monkeypatch) -> None:
    import batchalign.inference.languages.cantonese._cantonese_fa as module

    monkeypatch.setattr(module, "_model", None)
    monkeypatch.setattr(module, "_pc", None)

    assert default_cantonese_fa_host() is None


def test_load_cantonese_fa_populates_default_host(monkeypatch) -> None:
    import batchalign.inference.languages.cantonese._cantonese_fa as module

    fake_pc = object()
    monkeypatch.setitem(sys.modules, "pycantonese", fake_pc)
    monkeypatch.setattr(
        "batchalign.inference.fa.load_wave2vec_fa",
        lambda device_policy=None: {"device_policy": device_policy},
    )
    monkeypatch.setattr("batchalign.inference.audio.load_audio_file", lambda path: f"audio:{path}")
    monkeypatch.setattr(
        "batchalign.inference.fa.infer_wave2vec_fa",
        lambda model, audio, words: [("word", (0, 100))],
    )

    module.load_cantonese_fa("yue", None, device_policy="policy")
    host = module.default_cantonese_fa_host()

    assert host is not None
    assert host.model == {"device_policy": "policy"}
    assert host.romanizer is fake_pc
    assert host.load_audio_file("clip.wav") == "audio:clip.wav"
