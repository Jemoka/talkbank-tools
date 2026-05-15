"""Tests for the thin Python speaker inference boundary."""

from __future__ import annotations

from pathlib import Path
from types import ModuleType, SimpleNamespace
import sys
import wave

import numpy as np
import pytest
import torch

from batchalign.inference.speaker import (
    SpeakerSegment,
    _conv_scale_weights,
    _device_for_speaker_runtime,
    _get_pyannote_pipeline,
    _infer_nemo_speaker_from_audio_file,
    _parse_rttm_line,
    _resolve_speaker_config,
    _temporary_conv_scale_weights_override,
    _write_prepared_audio_wav,
    infer_nemo_speaker_prepared_audio,
    infer_pyannote_speaker_prepared_audio,
    infer_speaker_prepared_audio,
)


def test_parse_rttm_line_extracts_numeric_suffix() -> None:
    """RTTM parsing should normalize speaker labels into SPEAKER_N form."""

    segment = _parse_rttm_line(
        "SPEAKER file 1 0.25 0.50 <NA> <NA> speaker_7 <NA> <NA>"
    )

    assert segment == SpeakerSegment(start_ms=250, end_ms=750, speaker="SPEAKER_7")


def test_parse_rttm_line_rejects_short_records() -> None:
    """Malformed RTTM rows should fail before they reach document semantics."""

    with pytest.raises(ValueError, match="expected 10"):
        _parse_rttm_line("SPEAKER file 1 0.25")


def test_infer_speaker_prepared_audio_routes_pyannote(monkeypatch) -> None:
    """The high-level speaker entrypoint should forward Pyannote requests unchanged."""

    captured: dict[str, object] = {}

    def fake_pyannote(audio, sample_rate_hz, num_speakers):
        captured["audio"] = audio.copy()
        captured["sample_rate_hz"] = sample_rate_hz
        captured["num_speakers"] = num_speakers
        return [SpeakerSegment(start_ms=0, end_ms=100, speaker="SPEAKER_0")]

    monkeypatch.setattr(
        "batchalign.inference.speaker.infer_pyannote_speaker_prepared_audio",
        fake_pyannote,
    )

    response = infer_speaker_prepared_audio(
        np.asarray([0.1, 0.2, 0.3], dtype=np.float32),
        22050,
        num_speakers=3,
        engine="pyannote",
    )

    assert response.segments[0].speaker == "SPEAKER_0"
    assert captured["sample_rate_hz"] == 22050
    assert captured["num_speakers"] == 3
    assert np.array_equal(captured["audio"], np.asarray([0.1, 0.2, 0.3], dtype=np.float32))


def test_infer_speaker_prepared_audio_routes_nemo(monkeypatch) -> None:
    """The high-level speaker entrypoint should forward NeMo requests and device policy."""

    captured: dict[str, object] = {}

    def fake_nemo(audio, sample_rate_hz, num_speakers, *, device_policy=None):
        captured["audio"] = audio.copy()
        captured["sample_rate_hz"] = sample_rate_hz
        captured["num_speakers"] = num_speakers
        captured["device_policy"] = device_policy
        return [SpeakerSegment(start_ms=50, end_ms=125, speaker="SPEAKER_1")]

    monkeypatch.setattr(
        "batchalign.inference.speaker.infer_nemo_speaker_prepared_audio",
        fake_nemo,
    )

    response = infer_speaker_prepared_audio(
        np.asarray([0.4, 0.5], dtype=np.float32),
        16000,
        num_speakers=4,
        engine="nemo",
        device_policy="force-cpu",
    )

    assert response.segments[0].speaker == "SPEAKER_1"
    assert captured["sample_rate_hz"] == 16000
    assert captured["num_speakers"] == 4
    assert captured["device_policy"] == "force-cpu"
    assert np.array_equal(captured["audio"], np.asarray([0.4, 0.5], dtype=np.float32))


def test_temporary_conv_scale_weights_override_restores_original() -> None:
    """The NeMo compatibility override should restore the original attribute afterwards."""

    class _FakeMsddModule:
        def conv_scale_weights(self):
            return "original"

    module = _FakeMsddModule()
    original_func = module.conv_scale_weights.__func__

    with _temporary_conv_scale_weights_override(module):
        assert module.conv_scale_weights is not original_func

    assert module.conv_scale_weights.__func__ is original_func
    assert module.conv_scale_weights() == "original"


def test_temporary_conv_scale_weights_override_removes_temporary_attr() -> None:
    """Modules without the attribute should not keep the temporary override afterwards."""

    module = SimpleNamespace()

    with _temporary_conv_scale_weights_override(module):
        assert module.conv_scale_weights is _conv_scale_weights

    assert not hasattr(module, "conv_scale_weights")


def test_write_prepared_audio_wav_writes_mono_pcm(tmp_path: Path) -> None:
    """Prepared audio should be written as 16-bit mono WAV for downstream runtimes."""

    out = tmp_path / "prepared.wav"
    _write_prepared_audio_wav(np.asarray([0.0, 0.5, -0.5], dtype=np.float32), 8000, str(out))

    with wave.open(str(out), "rb") as handle:
        assert handle.getnchannels() == 1
        assert handle.getsampwidth() == 2
        assert handle.getframerate() == 8000
        assert handle.getnframes() == 3


def test_conv_scale_weights_computes_softmax_weights() -> None:
    """The NeMo MSDD override should produce per-speaker scale weights."""

    class _FakeMsddModule:
        conv = [object(), object()]
        conv_bn = [object(), object()]
        conv_repeat = 1
        batch_size = 1
        length = 2
        cnn_output_ch = 1
        emb_dim = 2
        num_spks = 2
        softmax = torch.nn.Softmax(dim=2)

        @staticmethod
        def conv_forward(conv_input, *, conv_module, bn_module, first_layer):
            return conv_input + (1.0 if first_layer else 2.0)

        @staticmethod
        def conv_to_linear(values):
            return values

        @staticmethod
        def dropout(values):
            return values

        @staticmethod
        def linear_to_weights(values):
            return values

    result = _conv_scale_weights(
        _FakeMsddModule(),
        torch.ones((1, 2, 1), dtype=torch.float32),
        torch.full((1, 2, 1), 2.0, dtype=torch.float32),
    )

    assert result.shape == (1, 2, 2, 2)
    assert torch.allclose(result.sum(dim=2), torch.ones((1, 2, 2)))


def test_resolve_speaker_config_points_at_repo_config() -> None:
    """The NeMo config helper should resolve the checked-in config file."""

    config_path = Path(_resolve_speaker_config())

    assert config_path.name == "speaker_config.yaml"
    assert config_path.exists()


@pytest.mark.parametrize(
    ("force_cpu", "cuda_available", "mps_available", "expected"),
    [
        (True, False, False, "cpu"),
        (False, True, False, "cuda"),
        # MPS must fall back to CPU: Pyannote produces wrong timestamps on MPS
        # (pyannote/pyannote-audio#1337, wontfix) and NeMo is CUDA-only.
        (False, False, True, "cpu"),
        (False, False, False, "cpu"),
    ],
)
def test_device_for_speaker_runtime_selects_expected_backend(
    monkeypatch,
    force_cpu: bool,
    cuda_available: bool,
    mps_available: bool,
    expected: str,
) -> None:
    """Speaker runtime device choice should respect force-CPU and hardware availability."""

    monkeypatch.setattr(
        "batchalign.device.force_cpu_preferred",
        lambda _device_policy: force_cpu,
    )
    monkeypatch.setattr("torch.cuda.is_available", lambda: cuda_available)
    monkeypatch.setattr("torch.backends.mps.is_available", lambda: mps_available)

    assert _device_for_speaker_runtime("policy") == expected


def test_get_pyannote_pipeline_caches_loaded_pipeline(monkeypatch) -> None:
    """Pyannote pipeline loading should happen only once per worker process."""

    loaded: list[str] = []

    class _FakePipeline:
        @staticmethod
        def from_pretrained(name: str):
            loaded.append(name)
            return {"pipeline": name}

    pyannote = ModuleType("pyannote")
    pyannote_audio = ModuleType("pyannote.audio")
    pyannote.audio = pyannote_audio
    pyannote_audio.Pipeline = _FakePipeline
    monkeypatch.setitem(sys.modules, "pyannote", pyannote)
    monkeypatch.setitem(sys.modules, "pyannote.audio", pyannote_audio)
    monkeypatch.setattr("batchalign.inference.speaker._PYANNOTE_PIPELINE", None)

    first = _get_pyannote_pipeline()
    second = _get_pyannote_pipeline()

    assert first is second
    assert loaded == ["talkbank/dia-fork"]


def test_infer_pyannote_speaker_prepared_audio_shapes_waveform_and_labels(monkeypatch) -> None:
    """Prepared-audio Pyannote inference should build mono waveforms and normalize labels."""

    captured: dict[str, object] = {}

    class _FakeResult:
        @staticmethod
        def itertracks(*, yield_label: bool):
            assert yield_label is True
            return iter(
                [
                    (SimpleNamespace(start=0.1, end=0.4), None, "speaker_2"),
                    (SimpleNamespace(start=0.5, end=0.8), None, "A"),
                ]
            )

    class _FakePipe:
        def __call__(self, payload, *, num_speakers: int):
            captured["waveform_shape"] = tuple(payload["waveform"].shape)
            captured["sample_rate"] = payload["sample_rate"]
            captured["num_speakers"] = num_speakers
            return _FakeResult()

    monkeypatch.setattr("batchalign.inference.speaker._get_pyannote_pipeline", lambda: _FakePipe())

    segments = infer_pyannote_speaker_prepared_audio(
        np.asarray([0.1, 0.2, 0.3], dtype=np.float64),
        16000,
        num_speakers=3,
    )

    assert captured == {
        "waveform_shape": (1, 3),
        "sample_rate": 16000,
        "num_speakers": 3,
    }
    assert segments == [
        SpeakerSegment(start_ms=100, end_ms=400, speaker="SPEAKER_2"),
        SpeakerSegment(start_ms=500, end_ms=800, speaker="SPEAKER_A"),
    ]


def test_infer_pyannote_speaker_prepared_audio_supports_diarize_output_api(monkeypatch) -> None:
    """Prepared-audio Pyannote inference should handle the newer DiarizeOutput shape."""

    captured: dict[str, object] = {}

    class _FakeDiarizeOutput:
        speaker_diarization = [
            (SimpleNamespace(start=0.1, end=0.4), "speaker_2"),
            (SimpleNamespace(start=0.5, end=0.8), "A"),
        ]

    class _FakePipe:
        def __call__(self, payload, *, num_speakers: int):
            captured["waveform_shape"] = tuple(payload["waveform"].shape)
            captured["sample_rate"] = payload["sample_rate"]
            captured["num_speakers"] = num_speakers
            return _FakeDiarizeOutput()

    monkeypatch.setattr("batchalign.inference.speaker._get_pyannote_pipeline", lambda: _FakePipe())

    segments = infer_pyannote_speaker_prepared_audio(
        np.asarray([0.1, 0.2, 0.3], dtype=np.float64),
        16000,
        num_speakers=3,
    )

    assert captured == {
        "waveform_shape": (1, 3),
        "sample_rate": 16000,
        "num_speakers": 3,
    }
    assert segments == [
        SpeakerSegment(start_ms=100, end_ms=400, speaker="SPEAKER_2"),
        SpeakerSegment(start_ms=500, end_ms=800, speaker="SPEAKER_A"),
    ]


def test_infer_nemo_speaker_prepared_audio_writes_temp_wav_and_forwards(monkeypatch) -> None:
    """Prepared-audio NeMo inference should materialize one WAV and delegate to file-path inference."""

    captured: dict[str, object] = {}

    def fake_write(audio: np.ndarray, sample_rate_hz: int, output_path: str) -> None:
        captured["audio"] = audio.copy()
        captured["sample_rate_hz"] = sample_rate_hz
        captured["basename"] = Path(output_path).name

    def fake_infer(audio_path: str, num_speakers: int, *, device_policy=None):
        captured["audio_path"] = Path(audio_path).name
        captured["num_speakers"] = num_speakers
        captured["device_policy"] = device_policy
        return [SpeakerSegment(start_ms=0, end_ms=100, speaker="SPEAKER_0")]

    monkeypatch.setattr("batchalign.inference.speaker._write_prepared_audio_wav", fake_write)
    monkeypatch.setattr("batchalign.inference.speaker._infer_nemo_speaker_from_audio_file", fake_infer)

    segments = infer_nemo_speaker_prepared_audio(
        np.asarray([0.4, 0.5], dtype=np.float32),
        22050,
        4,
        device_policy="force-cpu",
    )

    assert np.array_equal(captured["audio"], np.asarray([0.4, 0.5], dtype=np.float32))
    assert captured["sample_rate_hz"] == 22050
    assert captured["basename"] == "prepared_audio.wav"
    assert captured["audio_path"] == "prepared_audio.wav"
    assert captured["num_speakers"] == 4
    assert captured["device_policy"] == "force-cpu"
    assert segments == [SpeakerSegment(start_ms=0, end_ms=100, speaker="SPEAKER_0")]


def test_infer_nemo_speaker_from_audio_file_builds_manifest_and_parses_rttm(monkeypatch) -> None:
    """NeMo file-path inference should build the manifest, run diarization, and parse RTTM output."""

    captured: dict[str, object] = {}

    class _FakeAudioSegmentHandle:
        def set_channels(self, channels: int):
            captured["channels"] = channels
            return self

        def export(self, output_path: str, *, format: str) -> None:
            captured["mono_path"] = output_path
            Path(output_path).write_bytes(b"wav")

    class _FakeAudioSegment:
        @staticmethod
        def from_file(path: str):
            captured["source_audio"] = path
            return _FakeAudioSegmentHandle()

    class _FakeOmegaConf:
        @staticmethod
        def load(path: str):
            captured["config_path"] = path
            return SimpleNamespace(
                diarizer=SimpleNamespace(manifest_filepath=None, out_dir=None),
                device=None,
            )

    class _FakeNeuralDiarizer:
        def __init__(self, *, cfg) -> None:
            captured["manifest_path"] = cfg.diarizer.manifest_filepath
            captured["manifest_text"] = Path(cfg.diarizer.manifest_filepath).read_text(
                encoding="utf-8"
            ).strip()
            captured["out_dir"] = cfg.diarizer.out_dir
            captured["device"] = cfg.device

        def diarize(self) -> None:
            pred_dir = Path(captured["out_dir"]) / "pred_rttms"
            pred_dir.mkdir(parents=True, exist_ok=True)
            (pred_dir / "mono_file.rttm").write_text(
                "SPEAKER file 1 0.10 0.20 <NA> <NA> speaker_3 <NA> <NA>\n",
                encoding="utf-8",
            )

    class _FakeMSDDModule:
        pass

    omegaconf = ModuleType("omegaconf")
    omegaconf.OmegaConf = _FakeOmegaConf
    monkeypatch.setitem(sys.modules, "omegaconf", omegaconf)

    nemo = ModuleType("nemo")
    nemo_collections = ModuleType("nemo.collections")
    nemo_asr = ModuleType("nemo.collections.asr")
    nemo_models = ModuleType("nemo.collections.asr.models")
    nemo_msdd_models = ModuleType("nemo.collections.asr.models.msdd_models")
    nemo_modules = ModuleType("nemo.collections.asr.modules")
    nemo_msdd_diarizer = ModuleType("nemo.collections.asr.modules.msdd_diarizer")
    nemo.collections = nemo_collections
    nemo_collections.asr = nemo_asr
    nemo_asr.models = nemo_models
    nemo_asr.modules = nemo_modules
    nemo_models.msdd_models = nemo_msdd_models
    nemo_modules.msdd_diarizer = nemo_msdd_diarizer
    nemo_msdd_models.NeuralDiarizer = _FakeNeuralDiarizer
    nemo_msdd_diarizer.MSDD_module = _FakeMSDDModule
    monkeypatch.setitem(sys.modules, "nemo", nemo)
    monkeypatch.setitem(sys.modules, "nemo.collections", nemo_collections)
    monkeypatch.setitem(sys.modules, "nemo.collections.asr", nemo_asr)
    monkeypatch.setitem(sys.modules, "nemo.collections.asr.models", nemo_models)
    monkeypatch.setitem(sys.modules, "nemo.collections.asr.models.msdd_models", nemo_msdd_models)
    monkeypatch.setitem(sys.modules, "nemo.collections.asr.modules", nemo_modules)
    monkeypatch.setitem(sys.modules, "nemo.collections.asr.modules.msdd_diarizer", nemo_msdd_diarizer)

    pydub = ModuleType("pydub")
    pydub.AudioSegment = _FakeAudioSegment
    monkeypatch.setitem(sys.modules, "pydub", pydub)
    monkeypatch.setattr("batchalign.inference.speaker._resolve_speaker_config", lambda: "/tmp/speaker-config.yaml")
    monkeypatch.setattr("batchalign.inference.speaker._device_for_speaker_runtime", lambda _policy=None: "cpu")

    segments = _infer_nemo_speaker_from_audio_file(
        "/tmp/input.wav",
        3,
        device_policy="force-cpu",
    )

    assert '"audio_filepath"' in captured["manifest_text"]
    assert '"num_speakers": 3' in captured["manifest_text"]
    assert captured["config_path"] == "/tmp/speaker-config.yaml"
    assert captured["source_audio"] == "/tmp/input.wav"
    assert captured["channels"] == 1
    assert Path(captured["mono_path"]).name == "mono_file.wav"
    assert captured["device"] == "cpu"
    assert not hasattr(_FakeMSDDModule, "conv_scale_weights")
    assert segments == [SpeakerSegment(start_ms=100, end_ms=300, speaker="SPEAKER_3")]
