"""Tests for the Malayalam Wav2Vec2 CTC ASR backend."""

from __future__ import annotations

import sys
import types

import numpy as np


def test_pipeline_word_timestamps_are_projected_to_asr_output(monkeypatch):
    constructor: dict[str, object] = {}
    calls: list[tuple[object, dict[str, object]]] = []

    class FakePipeline:
        def __call__(self, audio, **kwargs):
            calls.append((audio, kwargs))
            return {
                "text": "നമസ്കാരം ലോകം",
                "chunks": [
                    {"text": "നമസ്കാരം ", "timestamp": (0.12, 0.82)},
                    {"text": "ലോകം", "timestamp": (0.90, 1.36)},
                ],
            }

    def fake_pipeline(task, model, **kwargs):
        constructor.update(task=task, model=model, **kwargs)
        return FakePipeline()

    monkeypatch.setitem(sys.modules, "transformers", types.SimpleNamespace(pipeline=fake_pipeline))

    class Record:
        def __init__(self, **kwargs):
            self.__dict__.update(kwargs)

    class AsrInput(Record):
        pass

    proto = types.ModuleType("batchalign._core.proto")
    proto.AsrInput = AsrInput
    proto.AsrOutput = type("AsrOutput", (Record,), {})
    proto.AsrSegment = type("AsrSegment", (Record,), {})
    proto.AsrWord = type("AsrWord", (Record,), {})
    monkeypatch.setitem(sys.modules, "batchalign._core.proto", proto)

    from batchalign.backends.asr.malayalam_wav2vec2 import (
        DEFAULT_MODEL,
        MalayalamWav2Vec2Backend,
    )

    samples = np.array([0.0, 0.25, -0.25, 0.0], dtype=np.float32)
    item = AsrInput(
        source_id="sample.wav",
        audio=Record(
            pcm_f32le=samples.tobytes(),
            sample_rate=48_000,
            channels=1,
            frame_count=len(samples),
        ),
        language=Record(kind="code", value="mal"),
        options=Record(),
    )

    backend = MalayalamWav2Vec2Backend()
    output = backend.call([item])[0]

    assert constructor == {
        "task": "automatic-speech-recognition",
        "model": DEFAULT_MODEL,
        "chunk_length_s": 30,
        "stride_length_s": (4, 2),
    }
    audio, call_kwargs = calls[0]
    assert audio["sampling_rate"] == 48_000
    np.testing.assert_array_equal(audio["array"], samples)
    assert call_kwargs == {"return_timestamps": "word"}

    assert output.source_id == "sample.wav"
    assert output.segments[0].text == "നമസ്കാരം ലോകം"
    assert [(word.text, word.start_ms, word.end_ms) for word in output.segments[0].words] == [
        ("നമസ്കാരം", 120, 820),
        ("ലോകം", 900, 1360),
    ]


def test_public_backend_export_is_asr():
    from batchalign.backends import ASR, MalayalamWav2Vec2Backend

    assert issubclass(MalayalamWav2Vec2Backend, ASR)


def test_clear_double_speed_timestamps_are_scaled_to_audio_duration():
    from batchalign.backends.asr._torch_audio import ctc_timestamp_scale

    chunks = [{"text": "വാക്ക്", "timestamp": (8.0, 19.8)}]

    assert ctc_timestamp_scale(chunks, duration_s=10.0) == 0.5
    assert ctc_timestamp_scale(chunks, duration_s=20.0) == 1.0
