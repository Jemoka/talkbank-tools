"""Tests for the thin Python AVQI inference boundary."""

from __future__ import annotations

from pathlib import Path
from types import ModuleType

import numpy as np

from batchalign.inference.avqi import (
    AvqiBatchItem,
    AvqiResponse,
    _build_sound,
    _calculate_features_from_sounds,
    _extract_voiced_segments,
    _mono_samples_from_file,
    calculate_avqi,
    infer_avqi_item,
    infer_avqi_prepared_audio,
)


class _FakeTensor:
    """Small tensor-like double for audio-loading helper tests."""

    def __init__(self, values: np.ndarray) -> None:
        self._values = values

    @property
    def shape(self) -> tuple[int, ...]:
        return self._values.shape

    def mean(self, dim: int):
        return _FakeTensor(self._values.mean(axis=dim))

    def squeeze(self, dim: int):
        return _FakeTensor(np.squeeze(self._values, axis=dim))

    def detach(self):
        return self

    def cpu(self):
        return self

    def numpy(self) -> np.ndarray:
        return self._values


def _install_fake_parselmouth(monkeypatch, *, call_fn=None, sound_class=None) -> None:
    """Install tiny fake parselmouth modules for AVQI helper tests."""

    parselmouth = ModuleType("parselmouth")
    if sound_class is not None:
        parselmouth.Sound = sound_class
    praat = ModuleType("parselmouth.praat")
    if call_fn is not None:
        praat.call = call_fn
    monkeypatch.setitem(__import__("sys").modules, "parselmouth", parselmouth)
    monkeypatch.setitem(__import__("sys").modules, "parselmouth.praat", praat)


def test_infer_avqi_item_validates_calculate_avqi_payload(monkeypatch) -> None:
    """The single-item adapter should validate and return the structured AVQI payload."""

    monkeypatch.setattr(
        "batchalign.inference.avqi.calculate_avqi",
        lambda cs_file, sv_file: {
            "avqi": 1.5,
            "cpps": 2.0,
            "hnr": 3.0,
            "shimmer_local": 4.0,
            "shimmer_local_db": 5.0,
            "slope": 6.0,
            "tilt": 7.0,
            "cs_file": Path(cs_file).name,
            "sv_file": Path(sv_file).name,
            "success": True,
        },
    )

    response = infer_avqi_item(AvqiBatchItem(cs_file="/tmp/cs.wav", sv_file="/tmp/sv.wav"))

    assert response.success is True
    assert response.cs_file == "cs.wav"
    assert response.sv_file == "sv.wav"


def test_infer_avqi_prepared_audio_success_path(monkeypatch) -> None:
    """Prepared-audio AVQI should forward built sounds into the feature calculator."""

    built_sounds: list[tuple[np.ndarray, int]] = []

    def fake_build_sound(samples: np.ndarray, sample_rate_hz: int):
        built_sounds.append((samples.copy(), sample_rate_hz))
        return f"sound:{sample_rate_hz}:{len(samples)}"

    monkeypatch.setattr("batchalign.inference.avqi._build_sound", fake_build_sound)
    monkeypatch.setattr(
        "batchalign.inference.avqi._calculate_features_from_sounds",
        lambda cs_sound, sv_sound: (
            4.2,
            {
                "cpps": 1.0,
                "hnr": 2.0,
                "shimmer_local": 3.0,
                "shimmer_local_db": 4.0,
                "slope": 5.0,
                "tilt": 6.0,
            },
        ),
    )

    response = infer_avqi_prepared_audio(
        np.asarray([0.1, 0.2], dtype=np.float32),
        16000,
        np.asarray([0.3, 0.4, 0.5], dtype=np.float32),
        22050,
        cs_label="cs-prepared",
        sv_label="sv-prepared",
    )

    assert built_sounds[0][1] == 16000
    assert built_sounds[1][1] == 22050
    assert response == AvqiResponse(
        avqi=4.2,
        cpps=1.0,
        hnr=2.0,
        shimmer_local=3.0,
        shimmer_local_db=4.0,
        slope=5.0,
        tilt=6.0,
        cs_file="cs-prepared",
        sv_file="sv-prepared",
        success=True,
        error=None,
    )


def test_infer_avqi_prepared_audio_returns_typed_error(monkeypatch) -> None:
    """Prepared-audio AVQI failures should stay structured and label-aware."""

    monkeypatch.setattr(
        "batchalign.inference.avqi._build_sound",
        lambda samples, sample_rate_hz: f"sound:{sample_rate_hz}:{len(samples)}",
    )

    def boom(cs_sound, sv_sound):
        raise RuntimeError(f"broken:{cs_sound}:{sv_sound}")

    monkeypatch.setattr("batchalign.inference.avqi._calculate_features_from_sounds", boom)

    response = infer_avqi_prepared_audio(
        np.asarray([0.1], dtype=np.float32),
        16000,
        np.asarray([0.2], dtype=np.float32),
        16000,
    )

    assert response.success is False
    assert response.error == "broken:sound:16000:1:sound:16000:1"


def test_calculate_avqi_uses_file_basenames_in_success_payload(monkeypatch) -> None:
    """File-path AVQI should pass prepared audio through with basename labels."""

    calls: list[tuple[str, str, str, str]] = []

    monkeypatch.setattr(
        "batchalign.inference.avqi._mono_samples_from_file",
        lambda path: (
            np.asarray([0.1, 0.2], dtype=np.float64),
            16000 if path.endswith("cs.wav") else 22050,
        ),
    )

    def fake_infer(cs_audio, cs_rate, sv_audio, sv_rate, *, cs_label, sv_label):
        calls.append((str(cs_audio.shape), str(cs_rate), cs_label, sv_label))
        return AvqiResponse(
            avqi=9.9,
            cpps=1.1,
            hnr=2.2,
            shimmer_local=3.3,
            shimmer_local_db=4.4,
            slope=5.5,
            tilt=6.6,
            cs_file=cs_label,
            sv_file=sv_label,
            success=True,
        )

    monkeypatch.setattr("batchalign.inference.avqi.infer_avqi_prepared_audio", fake_infer)

    payload = calculate_avqi("/tmp/session/cs.wav", "/tmp/session/sv.wav")

    assert calls == [("(2,)", "16000", "cs.wav", "sv.wav")]
    assert payload["success"] is True
    assert payload["cs_file"] == "cs.wav"
    assert payload["sv_file"] == "sv.wav"


def test_calculate_avqi_returns_structured_error_payload(monkeypatch) -> None:
    """Top-level AVQI file errors should stay structured and keep file names."""

    monkeypatch.setattr(
        "batchalign.inference.avqi._mono_samples_from_file",
        lambda path: (_ for _ in ()).throw(RuntimeError(f"cannot open {Path(path).name}")),
    )

    payload = calculate_avqi("/tmp/session/cs.wav", "/tmp/session/sv.wav")

    assert payload["success"] is False
    assert payload["cs_file"] == "cs.wav"
    assert payload["sv_file"] == "sv.wav"
    assert payload["error"] == "cannot open cs.wav"


def test_mono_samples_from_file_downmixes_stereo_and_build_sound_uses_float64(monkeypatch) -> None:
    """Audio helpers should downmix stereo and build parselmouth sounds with float64."""

    monkeypatch.setattr(
        "batchalign.inference.audio.load_audio",
        lambda _path: (
            _FakeTensor(np.asarray([[1.0, 3.0], [5.0, 7.0]], dtype=np.float32)),
            22050,
        ),
    )

    samples, sample_rate = _mono_samples_from_file("/tmp/test.wav")

    captured: list[tuple[np.ndarray, int]] = []

    class _FakeSound:
        def __init__(self, data: np.ndarray, *, sampling_frequency: int) -> None:
            captured.append((data, sampling_frequency))

    _install_fake_parselmouth(monkeypatch, sound_class=_FakeSound)
    sound = _build_sound(samples, sample_rate)

    assert np.array_equal(samples, np.asarray([3.0, 5.0], dtype=np.float64))
    assert sample_rate == 22050
    assert isinstance(sound, _FakeSound)
    assert captured[0][0].dtype == np.float64
    assert captured[0][1] == 22050


def test_extract_voiced_segments_keeps_loud_zero_crossing_windows(monkeypatch) -> None:
    """Voiced-segment extraction should concatenate windows that clear the threshold."""

    def fake_call(target, action, *args):
        if action == "Copy":
            return "original"
        if action == "Get sampling frequency":
            return 16000
        if target == "Create Sound" and action == "onlyVoice":
            return "onlyVoice"
        if action == "To TextGrid (silences)":
            return "textgrid"
        if action == "Extract intervals where":
            return "intervals"
        if action == "Concatenate" and target == "intervals":
            return "onlyLoud"
        if action == "Get power in air" and target == "onlyLoud":
            return 10.0
        if action == "Get end time":
            return 0.03
        if action == "Get start time":
            return 0.0
        if action == "Extract part":
            return "part1"
        if action == "Get power in air" and target == "part1":
            return 5.0
        if action == "Get nearest zero crossing":
            return 0.01
        if action == "Concatenate" and target == ["onlyVoice", "part1"]:
            return "voiced"
        raise AssertionError(f"unexpected Praat call: {target!r} {action!r} {args!r}")

    _install_fake_parselmouth(monkeypatch, call_fn=fake_call)

    assert _extract_voiced_segments("sound") == "voiced"


def test_extract_voiced_segments_ignores_zero_crossing_errors(monkeypatch) -> None:
    """Zero-crossing lookup failures should be ignored without aborting extraction."""

    def fake_call(target, action, *args):
        if action == "Copy":
            return "original"
        if action == "Get sampling frequency":
            return 16000
        if target == "Create Sound" and action == "onlyVoice":
            return "onlyVoice"
        if action == "To TextGrid (silences)":
            return "textgrid"
        if action == "Extract intervals where":
            return "intervals"
        if action == "Concatenate" and target == "intervals":
            return "onlyLoud"
        if action == "Get power in air" and target == "onlyLoud":
            return 10.0
        if action == "Get end time":
            return 0.03
        if action == "Get start time":
            return 0.0
        if action == "Extract part":
            return "part1"
        if action == "Get power in air" and target == "part1":
            return 5.0
        if action == "Get nearest zero crossing":
            raise RuntimeError("zero crossing exploded")
        raise AssertionError(f"unexpected Praat call: {target!r} {action!r} {args!r}")

    _install_fake_parselmouth(monkeypatch, call_fn=fake_call)

    assert _extract_voiced_segments("sound") == "onlyVoice"


def test_calculate_features_from_sounds_handles_trend_line_paths(monkeypatch) -> None:
    """Feature calculation should derive AVQI metrics from deterministic Praat outputs."""

    monkeypatch.setattr(
        "batchalign.inference.avqi._extract_voiced_segments",
        lambda cs_sound: "voiced_cs",
    )

    def fake_call(target, action, *args):
        if action == "Filter (stop Hann band)":
            return f"{target}_filtered"
        if action == "Get total duration":
            return 5.0
        if action == "Extract part":
            return "sv_part"
        if action == "Concatenate":
            return "concatenated"
        if action == "To PowerCepstrogram":
            return "powercepstrogram"
        if action == "Get CPPS":
            return 2.0
        if action == "To Ltas":
            return "ltas"
        if action == "Get slope" and target == "ltas":
            return 1.0
        if action == "Copy" and target == "ltas":
            return args[0]
        if action == "Compute trend line":
            return None
        if action == "Get slope" and target == "ltas_for_tilt":
            return 1.0
        if action == "Get slope" and target == "ltas_for_tilt2":
            return 1.0
        if action == "To PointProcess (periodic, cc)":
            return "pointprocess"
        if action == "Get shimmer (local)":
            return 0.12
        if action == "Get shimmer (local_dB)":
            return 0.34
        if action == "To Pitch (cc)":
            return "pitch"
        if action == "To PointProcess (cc)":
            return "pointprocess2"
        if action == "Voice report":
            return "Mean harmonics-to-noise ratio: 18.5"
        raise AssertionError(f"unexpected Praat call: {target!r} {action!r} {args!r}")

    _install_fake_parselmouth(monkeypatch, call_fn=fake_call)

    avqi, features = _calculate_features_from_sounds("cs", "sv")

    assert features == {
        "cpps": 2.0,
        "hnr": 18.5,
        "shimmer_local": 12.0,
        "shimmer_local_db": 0.34,
        "slope": 1.0,
        "tilt": 6.5,
    }
    expected_avqi = (
        4.152
        - (0.177 * 2.0)
        - (0.006 * 18.5)
        - (0.037 * 12.0)
        + (0.941 * 0.34)
        + (0.01 * 1.0)
        + (0.093 * 6.5)
    ) * 2.8902
    assert avqi == expected_avqi


def test_calculate_features_from_sounds_uses_copy_for_short_vowels_and_tilt_fallback(monkeypatch) -> None:
    """Short SV clips and trend-line failures should take the fallback paths."""

    monkeypatch.setattr(
        "batchalign.inference.avqi._extract_voiced_segments",
        lambda cs_sound: "voiced_cs",
    )

    def fake_call(target, action, *args):
        if action == "Filter (stop Hann band)":
            return f"{target}_filtered"
        if action == "Get total duration":
            return 2.0
        if action == "Copy" and target == "sv_filtered":
            return "sv_part"
        if action == "Concatenate":
            return "concatenated"
        if action == "To PowerCepstrogram":
            return "powercepstrogram"
        if action == "Get CPPS":
            return 1.5
        if action == "To Ltas":
            return "ltas"
        if action == "Get slope" and target == "ltas":
            return 2.0
        if action == "Copy" and target == "ltas":
            return "ltas_for_tilt"
        if action == "Compute trend line":
            raise RuntimeError("trend line exploded")
        if action == "To PointProcess (periodic, cc)":
            return "pointprocess"
        if action == "Get shimmer (local)":
            return 0.10
        if action == "Get shimmer (local_dB)":
            return 0.25
        if action == "To Pitch (cc)":
            return "pitch"
        if action == "To PointProcess (cc)":
            return "pointprocess2"
        if action == "Voice report":
            return "Mean harmonics-to-noise ratio: 12.0"
        raise AssertionError(f"unexpected Praat call: {target!r} {action!r} {args!r}")

    _install_fake_parselmouth(monkeypatch, call_fn=fake_call)

    avqi, features = _calculate_features_from_sounds("cs", "sv")

    assert features["tilt"] == 7.5
    expected_avqi = (
        4.152
        - (0.177 * 1.5)
        - (0.006 * 12.0)
        - (0.037 * 10.0)
        + (0.941 * 0.25)
        + (0.01 * 2.0)
        + (0.093 * 7.5)
    ) * 2.8902
    assert avqi == expected_avqi
