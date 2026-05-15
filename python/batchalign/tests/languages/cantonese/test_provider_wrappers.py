"""Unit tests for HK provider wrapper modules."""

from __future__ import annotations

from dataclasses import dataclass

import pytest

import batchalign.inference.languages.cantonese._aliyun_asr as aliyun_asr
from batchalign.inference.asr import AsrBatchItem, AsrElement, AsrMonologue, MonologueAsrResponse
import batchalign.inference.languages.cantonese._funaudio_asr as funaudio_asr
import batchalign.inference.languages.cantonese._tencent_asr as tencent_asr
from batchalign.worker._types import BatchInferRequest, InferTask


def _valid_request() -> BatchInferRequest:
    return BatchInferRequest(
        task=InferTask.ASR,
        lang="yue",
        items=[AsrBatchItem(audio_path="clip.wav", lang="yue").model_dump()],
    )


def _valid_response(lang: str = "yue") -> MonologueAsrResponse:
    return MonologueAsrResponse(
        lang=lang,
        monologues=[
            AsrMonologue(
                speaker=1,
                elements=[
                    AsrElement(value="好", ts=0.0, end_ts=0.2, type="text"),
                    AsrElement(value="   ", ts=None, end_ts=None, type="text"),
                ],
            )
        ],
    )


@dataclass
class _FakeFunRecognizer:
    payload: tuple[dict[str, object], list[dict[str, object]]] | None = None
    error: Exception | None = None

    def transcribe(self, _audio_path: str):
        if self.error is not None:
            raise self.error
        if self.payload is not None:
            return self.payload
        return (
            {
                "monologues": [
                    {
                        "speaker": 1,
                        "elements": [
                            {"value": "好", "ts": 0.0, "end_ts": 0.2, "type": "text"},
                            {"value": "   ", "ts": None, "end_ts": None, "type": "text"},
                        ],
                    }
                ]
            },
            [],
        )


@dataclass
class _FakeTencentRecognizer:
    error: Exception | None = None

    def transcribe(self, _audio_path: str, num_speakers: int = 0):
        if self.error is not None:
            raise self.error
        return [{"speaker_count": num_speakers}]

    def monologues(self, _details):
        return {
            "monologues": [
                {
                    "speaker": 1,
                    "elements": [
                        {"value": "好", "ts": 0.0, "end_ts": 0.2, "type": "text"},
                        {"value": "   ", "ts": None, "end_ts": None, "type": "text"},
                    ],
                }
            ]
        }


def test_load_funaudio_asr_applies_engine_overrides(monkeypatch) -> None:
    created: dict[str, object] = {}

    class FakeRecognizer:
        def __init__(self, *, lang: str, model: str, device: str) -> None:
            created.update({"lang": lang, "model": model, "device": device})

    monkeypatch.setattr(funaudio_asr, "FunAudioRecognizer", FakeRecognizer)
    monkeypatch.setattr(funaudio_asr, "_recognizer", None)

    funaudio_asr.load_funaudio_asr(
        "yue",
        {"funaudio_model": "custom-model", "funaudio_device": "cuda"},
    )

    assert created == {
        "lang": "yue",
        "model": "custom-model",
        "device": "cuda",
    }


def test_infer_funaudio_asr_requires_loaded_recognizer(monkeypatch) -> None:
    monkeypatch.setattr(funaudio_asr, "_recognizer", None)

    response = funaudio_asr.infer_funaudio_asr(_valid_request())

    assert response.results[0].error == "FunAudio ASR not loaded — call load_funaudio_asr first"


def test_infer_funaudio_asr_rejects_invalid_items(monkeypatch) -> None:
    monkeypatch.setattr(funaudio_asr, "_recognizer", _FakeFunRecognizer())
    request = BatchInferRequest(task=InferTask.ASR, lang="yue", items=[{}])

    response = funaudio_asr.infer_funaudio_asr(request)

    assert response.results[0].error == "Invalid AsrBatchItem"


def test_infer_funaudio_asr_surfaces_runtime_errors(monkeypatch) -> None:
    monkeypatch.setattr(
        funaudio_asr,
        "_recognizer",
        _FakeFunRecognizer(error=RuntimeError("boom")),
    )

    response = funaudio_asr.infer_funaudio_asr(_valid_request())

    assert response.results[0].error == "boom"


def test_infer_funaudio_asr_returns_result_payload(monkeypatch) -> None:
    monotonic_values = iter([0.0, 1.5])
    monkeypatch.setattr(funaudio_asr, "_recognizer", _FakeFunRecognizer())
    monkeypatch.setattr(
        "batchalign.inference.languages.cantonese._funaudio_asr.time.monotonic",
        lambda: next(monotonic_values),
    )

    response = funaudio_asr.infer_funaudio_asr(_valid_request())

    assert response.results[0].error is None
    assert response.results[0].result["monologues"][0]["elements"][0]["value"] == "好"
    assert response.results[0].elapsed_s == 1.5


def test_funaudio_transcribe_to_monologues_requires_loaded_recognizer(monkeypatch) -> None:
    monkeypatch.setattr(funaudio_asr, "_recognizer", None)

    with pytest.raises(RuntimeError, match="not initialized"):
        funaudio_asr._transcribe_to_monologues(AsrBatchItem(audio_path="clip.wav"))


def test_infer_funaudio_asr_v2_filters_blank_elements(monkeypatch) -> None:
    monkeypatch.setattr(funaudio_asr, "_recognizer", _FakeFunRecognizer())

    response = funaudio_asr.infer_funaudio_asr_v2(
        AsrBatchItem(audio_path="clip.wav", lang="yue")
    )

    assert response.lang == "yue"
    assert [element.value for element in response.monologues[0].elements] == ["好"]


def test_infer_tencent_asr_requires_loaded_provider(monkeypatch) -> None:
    monkeypatch.setattr(tencent_asr, "_recognizer", None)

    response = tencent_asr.infer_tencent_asr(_valid_request())

    assert response.results[0].error == "Tencent ASR provider not loaded"


def test_load_tencent_asr_stores_recognizer(monkeypatch) -> None:
    created: dict[str, object] = {}

    class FakeRecognizer:
        def __init__(self, *, lang: str, config=None) -> None:
            created.update({"lang": lang, "config": config})

    monkeypatch.setattr(tencent_asr, "TencentRecognizer", FakeRecognizer)

    tencent_asr.load_tencent_asr("yue", None, config={"cfg": True})

    assert created == {"lang": "yue", "config": {"cfg": True}}
    assert tencent_asr._lang == "yue"


def test_infer_tencent_asr_rejects_invalid_items(monkeypatch) -> None:
    monkeypatch.setattr(tencent_asr, "_recognizer", _FakeTencentRecognizer())
    request = BatchInferRequest(task=InferTask.ASR, lang="yue", items=[{}])

    response = tencent_asr.infer_tencent_asr(request)

    assert response.results[0].error == "Invalid AsrBatchItem"


def test_infer_tencent_asr_surfaces_runtime_errors(monkeypatch) -> None:
    monkeypatch.setattr(
        tencent_asr,
        "_recognizer",
        _FakeTencentRecognizer(error=RuntimeError("boom")),
    )

    response = tencent_asr.infer_tencent_asr(_valid_request())

    assert response.results[0].error == "boom"


def test_infer_tencent_asr_returns_result_payload(monkeypatch) -> None:
    monotonic_values = iter([0.0, 2.0])
    monkeypatch.setattr(tencent_asr, "_recognizer", _FakeTencentRecognizer())
    monkeypatch.setattr(
        "batchalign.inference.languages.cantonese._tencent_asr.time.monotonic",
        lambda: next(monotonic_values),
    )

    response = tencent_asr.infer_tencent_asr(_valid_request())

    assert response.results[0].error is None
    assert response.results[0].result["monologues"][0]["elements"][0]["value"] == "好"
    assert response.results[0].elapsed_s == 2.0


def test_tencent_transcribe_to_monologues_requires_loaded_recognizer(monkeypatch) -> None:
    monkeypatch.setattr(tencent_asr, "_recognizer", None)

    with pytest.raises(RuntimeError, match="not initialized"):
        tencent_asr._transcribe_to_monologues(AsrBatchItem(audio_path="clip.wav"))


def test_infer_tencent_asr_v2_filters_blank_elements(monkeypatch) -> None:
    monkeypatch.setattr(tencent_asr, "_recognizer", _FakeTencentRecognizer())

    response = tencent_asr.infer_tencent_asr_v2(
        AsrBatchItem(audio_path="clip.wav", lang="yue")
    )

    assert response.lang == "yue"
    assert [element.value for element in response.monologues[0].elements] == ["好"]


def test_infer_aliyun_asr_v2_delegates_to_single_item_helper(monkeypatch) -> None:
    expected = _valid_response()
    monkeypatch.setattr(
        aliyun_asr,
        "_transcribe_to_monologues",
        lambda _path: expected,
    )

    response = aliyun_asr.infer_aliyun_asr_v2(AsrBatchItem(audio_path="clip.wav"))

    assert response == expected


def test_infer_aliyun_asr_requires_load(monkeypatch) -> None:
    monkeypatch.setattr(aliyun_asr, "_ak_id", "")

    with pytest.raises(RuntimeError, match="load_aliyun_asr"):
        aliyun_asr.infer_aliyun_asr(_valid_request())


def test_infer_aliyun_asr_rejects_invalid_items(monkeypatch) -> None:
    monkeypatch.setattr(aliyun_asr, "_ak_id", "loaded")
    request = BatchInferRequest(task=InferTask.ASR, lang="yue", items=[{}])

    response = aliyun_asr.infer_aliyun_asr(request)

    assert response.results[0].error == "Invalid AsrBatchItem"


def test_infer_aliyun_asr_surfaces_runtime_errors(monkeypatch) -> None:
    monkeypatch.setattr(aliyun_asr, "_ak_id", "loaded")
    monkeypatch.setattr(
        aliyun_asr,
        "_transcribe_to_monologues",
        lambda _path: (_ for _ in ()).throw(RuntimeError("boom")),
    )

    response = aliyun_asr.infer_aliyun_asr(_valid_request())

    assert response.results[0].error == "boom"


def test_infer_aliyun_asr_returns_result_payload(monkeypatch) -> None:
    monotonic_values = iter([0.0, 3.0])
    expected = _valid_response()
    monkeypatch.setattr(aliyun_asr, "_ak_id", "loaded")
    monkeypatch.setattr(
        aliyun_asr,
        "_transcribe_to_monologues",
        lambda _path: expected,
    )
    monkeypatch.setattr(
        "batchalign.inference.languages.cantonese._aliyun_asr.time.monotonic",
        lambda: next(monotonic_values),
    )

    response = aliyun_asr.infer_aliyun_asr(_valid_request())

    assert response.results[0].error is None
    assert response.results[0].result["monologues"][0]["elements"][0]["value"] == "好"
    assert response.results[0].elapsed_s == 3.0
