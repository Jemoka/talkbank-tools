"""Qwen3 ASR/FA loader and public-surface contracts."""

from __future__ import annotations

import sys
from types import SimpleNamespace

from batchalign.backends.asr.qwen3_asr import Qwen3AsrBackend
from batchalign.backends.fa.qwen3_fa import Qwen3FaBackend
from batchalign.lang import LanguageCode


class _Torch:
    bfloat16 = "bfloat16"
    float16 = "float16"
    float32 = "float32"


def test_standalone_fa_loads_only_the_typed_forced_aligner(monkeypatch):
    calls: list[tuple[str, dict[str, object]]] = []

    class _ForcedAligner:
        @classmethod
        def from_pretrained(cls, model_id, **kwargs):
            calls.append((model_id, kwargs))
            return cls()

    class _AsrModel:
        @classmethod
        def from_pretrained(cls, *_args, **_kwargs):
            raise AssertionError("standalone FA must not load Qwen3ASRModel")

    monkeypatch.setitem(sys.modules, "torch", _Torch())
    monkeypatch.setitem(
        sys.modules,
        "qwen_asr",
        SimpleNamespace(
            Qwen3ForcedAligner=_ForcedAligner,
            Qwen3ASRModel=_AsrModel,
        ),
    )

    backend = Qwen3FaBackend(model_id="Qwen/test-aligner", device="cpu")

    assert isinstance(backend._aligner, _ForcedAligner)
    assert calls == [
        ("Qwen/test-aligner", {"dtype": "float32", "device_map": "cpu"})
    ]


def test_asr_loads_the_companion_forced_aligner_for_word_timestamps(monkeypatch):
    calls: list[tuple[str, dict[str, object]]] = []

    class _AsrModel:
        @classmethod
        def from_pretrained(cls, model_id, **kwargs):
            calls.append((model_id, kwargs))
            return cls()

    monkeypatch.setitem(sys.modules, "torch", _Torch())
    monkeypatch.setitem(
        sys.modules,
        "qwen_asr",
        SimpleNamespace(Qwen3ASRModel=_AsrModel),
    )

    backend = Qwen3AsrBackend(
        language=LanguageCode.from_str("yue"),
        model_id="Qwen/test-asr",
        device="cpu",
    )

    assert isinstance(backend._model, _AsrModel)
    assert calls[0][0] == "Qwen/test-asr"
    assert calls[0][1]["forced_aligner"] == "Qwen/Qwen3-ForcedAligner-0.6B"
    assert calls[0][1]["torch_dtype"] == "float32"


def test_qwen_backends_are_available_through_public_typed_surface():
    import batchalign

    assert batchalign.Qwen3AsrBackend is Qwen3AsrBackend
    assert batchalign.Qwen3FaBackend is Qwen3FaBackend
    assert "Qwen3AsrBackend" in batchalign.__all__
    assert "Qwen3FaBackend" in batchalign.__all__
