# affects: python/batchalign/inference/asr/**
"""Tests for ASR inference boundary models and Whisper helpers.

Exercises the Pydantic models and the two remaining local Whisper helpers.
No ML models are required — the tests use structured runtime doubles.
"""

from __future__ import annotations

import json
from types import ModuleType
import sys

import numpy as np
import torch

from batchalign.inference.asr import (
    AsrBatchItem,
    AsrElement,
    AsrMonologue,
    MonologueAsrResponse,
    WhisperChunk,
    WhisperChunksAsrResponse,
    _infer_whisper,
    infer_whisper_prepared_audio,
    iso3_to_language_name,
    load_whisper_asr,
)


# ---------------------------------------------------------------------------
# Pydantic model tests
# ---------------------------------------------------------------------------


class TestAsrModels:
    """Test Pydantic model serialization/validation."""

    def test_asr_batch_item_defaults(self) -> None:
        item = AsrBatchItem(audio_path="/tmp/test.wav")
        assert item.lang == "eng"
        assert item.num_speakers == 1
        assert item.rev_job_id is None

    def test_asr_element_with_all_fields(self) -> None:
        element = AsrElement(
            value="hello",
            ts=1.0,
            end_ts=2.0,
            type="text",
            confidence=0.95,
        )
        data = element.model_dump()
        assert data["value"] == "hello"
        assert data["ts"] == 1.0
        assert data["type"] == "text"

    def test_asr_monologue_response_roundtrip(self) -> None:
        response = MonologueAsrResponse(
            lang="eng",
            monologues=[
                AsrMonologue(
                    speaker=1,
                    elements=[
                        AsrElement(value="hello", ts=0.0, end_ts=0.5),
                        AsrElement(value="world", ts=0.5, end_ts=1.0),
                    ],
                )
            ],
        )
        data = json.loads(response.model_dump_json())
        back = MonologueAsrResponse.model_validate(data)
        assert back.kind == "monologues"
        assert len(back.monologues) == 1
        assert len(back.monologues[0].elements) == 2
        assert back.monologues[0].elements[0].value == "hello"

    def test_whisper_chunks_response_roundtrip(self) -> None:
        response = WhisperChunksAsrResponse(
            lang="eng",
            text="hello world",
            chunks=[
                WhisperChunk(text="hello", timestamp=(0.0, 0.5)),
                WhisperChunk(text="world", timestamp=(0.5, 1.0)),
            ],
        )
        data = json.loads(response.model_dump_json())
        back = WhisperChunksAsrResponse.model_validate(data)
        assert back.kind == "whisper_chunks"
        assert len(back.chunks) == 2
        assert back.chunks[0].text == "hello"
        assert back.lang == "eng"


class TestWhisperPathBoundary:
    """Verify the local Whisper adapter stays thin at the Python boundary."""

    def test_iso3_to_language_name_maps_whisper_names(self) -> None:
        """Prepared-audio Whisper should share the same ISO3->name mapping as load."""
        assert iso3_to_language_name("eng") == "english"
        assert iso3_to_language_name("yue") == "Cantonese"

    def test_iso3_to_language_name_rejects_unknown_codes(self) -> None:
        """Unknown ISO 639-3 codes raise ValueError (no silent English fallback)."""
        import pytest

        with pytest.raises(ValueError, match="Unrecognized ISO 639-3"):
            iso3_to_language_name("zzz")

    def test_iso3_to_language_name_auto_returns_auto(self) -> None:
        """``--lang auto`` should map to the sentinel ``"auto"`` language name."""
        assert iso3_to_language_name("auto") == "auto"

    def test_infer_whisper_passes_source_path_to_pipeline(self) -> None:
        """The wrapper should forward the source path, not decode audio itself."""

        class _FakeWhisperHandle:
            """Minimal structured test double for the local Whisper handle."""

            def __init__(self) -> None:
                self.lang = "english"
                self.sample_rate = 16000
                self.calls: list[tuple[object, int, object]] = []

            def gen_kwargs(self, lang: str) -> dict[str, object]:
                return {"language": lang}

            def __call__(
                self,
                audio: object,
                *,
                batch_size: int = 1,
                generate_kwargs: object = None,
            ) -> dict[str, object]:
                self.calls.append((audio, batch_size, generate_kwargs))
                return {
                    "text": "hello world",
                    "chunks": [
                        {"text": "hello", "timestamp": (0.0, 0.5)},
                        {"text": "world", "timestamp": (0.5, 1.0)},
                    ],
                }

        model = _FakeWhisperHandle()
        item = AsrBatchItem(audio_path="/tmp/input.wav", lang="eng")

        response = _infer_whisper(model, item)  # type: ignore[arg-type]

        assert response.kind == "whisper_chunks"
        assert response.text == "hello world"
        assert model.calls == [
            ("/tmp/input.wav", 1, {"language": "english"})
        ]

    def test_infer_whisper_prepared_audio_passes_waveform_directly(self) -> None:
        """The prepared-audio helper should not reopen media paths in Python."""

        class _FakeWhisperHandle:
            """Small typed test double for the prepared-audio helper."""

            def __init__(self) -> None:
                self.sample_rate = 16000
                self.calls: list[tuple[object, int, object]] = []

            def gen_kwargs(self, lang: str) -> dict[str, object]:
                return {"language": lang}

            def __call__(
                self,
                audio: object,
                *,
                batch_size: int = 1,
                generate_kwargs: object = None,
            ) -> dict[str, object]:
                self.calls.append((audio, batch_size, generate_kwargs))
                return {
                    "text": "hello",
                    "chunks": [
                        {"text": "hello", "timestamp": (0.0, 0.5)},
                    ],
                }

        model = _FakeWhisperHandle()
        waveform = np.asarray([0.1, 0.2, 0.3], dtype=np.float32)

        response = infer_whisper_prepared_audio(model, waveform, "eng")  # type: ignore[arg-type]

        assert response.text == "hello"
        assert response.chunks[0].start_s == 0.0
        assert len(model.calls) == 1
        prepared_input, batch_size, generate_kwargs = model.calls[0]
        assert batch_size == 1
        assert generate_kwargs == {"language": "english"}
        assert isinstance(prepared_input, dict)
        assert prepared_input["sampling_rate"] == 16000
        assert np.array_equal(prepared_input["raw"], waveform)

    def test_infer_whisper_auto_detect_omits_language(self) -> None:
        """With ``--lang auto``, the Whisper pipeline must NOT receive a language hint."""

        class _FakeWhisperHandle:
            """Test double that records generate_kwargs for auto-detect assertion."""

            def __init__(self) -> None:
                self.lang = "auto"
                self.sample_rate = 16000
                self.calls: list[tuple[object, int, object]] = []

            def gen_kwargs(self, lang: str) -> dict[str, object]:
                from batchalign.inference.types import WhisperASRHandle

                handle = WhisperASRHandle(
                    pipe=None, config="cfg", lang="auto", sample_rate=16000
                )
                return handle.gen_kwargs(lang)

            def __call__(
                self,
                audio: object,
                *,
                batch_size: int = 1,
                generate_kwargs: object = None,
            ) -> dict[str, object]:
                self.calls.append((audio, batch_size, generate_kwargs))
                return {
                    "text": "hola hello",
                    "chunks": [
                        {"text": "hola", "timestamp": (0.0, 0.5)},
                        {"text": "hello", "timestamp": (0.5, 1.0)},
                    ],
                }

        model = _FakeWhisperHandle()
        item = AsrBatchItem(audio_path="/tmp/bilingual.wav", lang="auto")

        response = _infer_whisper(model, item)  # type: ignore[arg-type]

        assert response.kind == "whisper_chunks"
        assert response.text == "hola hello"
        _, _, gen_kw = model.calls[0]
        assert "language" not in gen_kw, (
            "auto-detect must omit 'language' from generate_kwargs"
        )

    def test_infer_whisper_prepared_audio_auto_detect(self) -> None:
        """Prepared-audio path with ``lang='auto'`` must omit language hint."""

        class _FakeWhisperHandle:
            """Test double for prepared-audio auto-detect path."""

            def __init__(self) -> None:
                self.sample_rate = 16000
                self.calls: list[tuple[object, int, object]] = []

            def gen_kwargs(self, lang: str) -> dict[str, object]:
                from batchalign.inference.types import WhisperASRHandle

                handle = WhisperASRHandle(
                    pipe=None, config="cfg", lang="auto", sample_rate=16000
                )
                return handle.gen_kwargs(lang)

            def __call__(
                self,
                audio: object,
                *,
                batch_size: int = 1,
                generate_kwargs: object = None,
            ) -> dict[str, object]:
                self.calls.append((audio, batch_size, generate_kwargs))
                return {
                    "text": "bilingual output",
                    "chunks": [
                        {"text": "bilingual output", "timestamp": (0.0, 1.0)},
                    ],
                }

        model = _FakeWhisperHandle()
        waveform = np.asarray([0.1, 0.2, 0.3], dtype=np.float32)

        response = infer_whisper_prepared_audio(model, waveform, "auto")  # type: ignore[arg-type]

        assert response.text == "bilingual output"
        _, _, gen_kw = model.calls[0]
        assert "language" not in gen_kw, (
            "auto-detect must omit 'language' from generate_kwargs"
        )


class _FakeGenerationConfig:
    def __init__(self, base: str) -> None:
        self.base = base
        self.no_repeat_ngram_size: int | None = None
        self.use_cache: bool | None = None
        self.no_timestamps_token_id: int | None = None
        self.alignment_heads: list[list[int]] | None = None

    @classmethod
    def from_pretrained(cls, base: str) -> "_FakeGenerationConfig":
        return cls(base)


class _FakeTokenizer:
    calls: list[str] = []

    @classmethod
    def from_pretrained(cls, base: str) -> str:
        cls.calls.append(base)
        return f"tokenizer:{base}"


class _FakeProcessor:
    calls: list[str] = []

    @classmethod
    def from_pretrained(cls, base: str) -> str:
        cls.calls.append(base)
        return f"processor:{base}"


class _FakePipeModel:
    def __init__(self) -> None:
        self.eval_called = False

    def eval(self) -> None:
        self.eval_called = True


class _FakePipe:
    def __init__(self) -> None:
        self.model = _FakePipeModel()


def _install_fake_transformers(monkeypatch, pipeline_impl):
    _FakeTokenizer.calls = []
    _FakeProcessor.calls = []
    module = ModuleType("transformers")
    module.GenerationConfig = _FakeGenerationConfig
    module.WhisperProcessor = _FakeProcessor
    module.WhisperTokenizer = _FakeTokenizer
    module.pipeline = pipeline_impl
    monkeypatch.setitem(sys.modules, "transformers", module)


class TestWhisperLoader:
    def test_load_whisper_asr_prefers_cuda_when_available(self, monkeypatch) -> None:
        pipeline_calls: list[tuple[str, dict[str, object]]] = []
        pipe = _FakePipe()
        bind_calls: list[object] = []

        def fake_pipeline(task: str, **kwargs: object) -> _FakePipe:
            pipeline_calls.append((task, kwargs))
            return pipe

        _install_fake_transformers(monkeypatch, fake_pipeline)
        monkeypatch.setattr(
            "batchalign.inference.audio.bind_whisper_token_timestamp_extractor",
            lambda model: bind_calls.append(model),
        )
        monkeypatch.setattr(
            "batchalign.device.force_cpu_preferred",
            lambda _policy=None: False,
        )
        monkeypatch.setattr(torch.cuda, "is_available", lambda: True)
        monkeypatch.setattr(torch.backends.mps, "is_available", lambda: False)

        handle = load_whisper_asr(
            model="cuda-model",
            base="cuda-base",
            language="english",
            target_sample_rate=44100,
            device_policy="policy",
        )

        assert handle.lang == "english"
        assert handle.sample_rate == 44100
        assert handle.config.base == "cuda-base"
        assert handle.config.no_repeat_ngram_size == 4
        assert handle.config.use_cache is True
        assert pipeline_calls == [
            (
                "automatic-speech-recognition",
                {
                    "model": "cuda-model",
                    "tokenizer": "tokenizer:cuda-base",
                    "chunk_length_s": 25,
                    "stride_length_s": 3,
                    "device": torch.device("cuda"),
                    "torch_dtype": torch.float16,
                    "return_timestamps": True,
                },
            )
        ]
        assert _FakeTokenizer.calls == ["cuda-base"]
        assert _FakeProcessor.calls == ["cuda-base"]
        assert bind_calls == [pipe.model]
        assert pipe.model.eval_called is True

    def test_load_whisper_asr_ignores_mps_and_applies_cantonese_overrides(
        self,
        monkeypatch,
    ) -> None:
        pipeline_calls: list[tuple[str, dict[str, object]]] = []
        pipe = _FakePipe()

        def fake_pipeline(task: str, **kwargs: object) -> _FakePipe:
            pipeline_calls.append((task, kwargs))
            return pipe

        _install_fake_transformers(monkeypatch, fake_pipeline)
        monkeypatch.setattr(
            "batchalign.inference.audio.bind_whisper_token_timestamp_extractor",
            lambda _model: None,
        )
        monkeypatch.setattr(
            "batchalign.device.force_cpu_preferred",
            lambda _policy=None: False,
        )
        monkeypatch.setattr(torch.cuda, "is_available", lambda: False)
        monkeypatch.setattr(torch.backends.mps, "is_available", lambda: True)

        handle = load_whisper_asr(
            model="mps-model",
            base="mps-base",
            language="Cantonese",
            target_sample_rate=22050,
            device_policy="policy",
        )

        assert handle.config.no_timestamps_token_id == 50363
        assert handle.config.alignment_heads == [
            [5, 3], [5, 9], [8, 0], [8, 4], [8, 8],
            [9, 0], [9, 7], [9, 9], [10, 5],
        ]
        # MPS excluded since 2026-04-05 (AGXG14X kernel deadlock) — even with
        # MPS available, the loader selects CPU. Cantonese config overrides
        # (alignment_heads, no_timestamps_token_id) are language-dependent, not
        # device-dependent, and must still be applied.
        assert pipeline_calls[0][1]["device"] == torch.device("cpu")
        assert pipeline_calls[0][1]["torch_dtype"] == torch.float32

    def test_load_whisper_asr_uses_cpu_when_no_accelerator_exists(
        self,
        monkeypatch,
    ) -> None:
        pipeline_calls: list[tuple[str, dict[str, object]]] = []

        def fake_pipeline(task: str, **kwargs: object) -> _FakePipe:
            pipeline_calls.append((task, kwargs))
            return _FakePipe()

        _install_fake_transformers(monkeypatch, fake_pipeline)
        monkeypatch.setattr(
            "batchalign.inference.audio.bind_whisper_token_timestamp_extractor",
            lambda _model: None,
        )
        monkeypatch.setattr(
            "batchalign.device.force_cpu_preferred",
            lambda _policy=None: False,
        )
        monkeypatch.setattr(torch.cuda, "is_available", lambda: False)
        monkeypatch.setattr(torch.backends.mps, "is_available", lambda: False)

        handle = load_whisper_asr(device_policy="policy")

        assert handle.lang == "english"
        assert pipeline_calls[0][1]["device"] == torch.device("cpu")
        assert pipeline_calls[0][1]["torch_dtype"] == torch.float32

    # test_load_whisper_asr_falls_back_when_pipeline_rejects_bfloat16 removed:
    # The try/except TypeError fallback was eliminated because after the MPS
    # exclusion (2026-04-05), both branches used identical dtypes (float16 on
    # CUDA, float32 on CPU). The fallback originally existed to retry with
    # float16 when older HuggingFace pipelines rejected bfloat16.
