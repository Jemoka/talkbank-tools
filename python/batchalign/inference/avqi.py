"""AVQI (Acoustic Voice Quality Index): paired audio -> voice quality metrics."""

from __future__ import annotations

import logging
import re
from pathlib import Path
from typing import TYPE_CHECKING, NotRequired, TypedDict

import numpy as np
from pydantic import BaseModel

from batchalign.inference._domain_types import AudioPath

if TYPE_CHECKING:
    import parselmouth

L = logging.getLogger("batchalign.worker")


class AvqiBatchItem(BaseModel):
    """A single AVQI inference request."""

    cs_file: AudioPath
    sv_file: AudioPath


class AvqiResponse(BaseModel):
    """Raw AVQI metric payload returned by the Python model host."""

    avqi: float
    cpps: float
    hnr: float
    shimmer_local: float
    shimmer_local_db: float
    slope: float
    tilt: float
    cs_file: str
    sv_file: str
    success: bool
    error: str | None = None


class AVQIResult(TypedDict, total=False):
    """Structured return payload for AVQI analysis."""

    avqi: float
    cpps: float
    hnr: float
    shimmer_local: float
    shimmer_local_db: float
    slope: float
    tilt: float
    cs_file: str
    sv_file: str
    error: NotRequired[str]
    success: bool


def _extract_voiced_segments(sound: parselmouth.Sound) -> parselmouth.Sound:
    """Extract voiced segments from audio using Praat."""
    from parselmouth.praat import call

    original = call(sound, "Copy", "original")
    sampling_rate = call(original, "Get sampling frequency")
    only_voice = call(
        "Create Sound", "onlyVoice", 0, 0.001, sampling_rate, "0"
    )
    textgrid = call(
        original,
        "To TextGrid (silences)",
        50,
        0.003,
        -25,
        0.1,
        0.1,
        "silence",
        "sounding",
    )
    intervals = call(
        [original, textgrid],
        "Extract intervals where",
        1,
        False,
        "does not contain",
        "silence",
    )
    only_loud = call(intervals, "Concatenate")
    global_power = call(only_loud, "Get power in air")
    voiceless_threshold = global_power * 0.3
    signal_end = call(only_loud, "Get end time")
    window_left = call(only_loud, "Get start time")
    window_width = 0.03
    while window_left + window_width <= signal_end:
        part = call(
            only_loud,
            "Extract part",
            window_left,
            window_left + window_width,
            "Rectangular",
            1.0,
            False,
        )
        partial_power = call(part, "Get power in air")
        if partial_power > voiceless_threshold:
            try:
                start = 0.0025
                start_zero = call(part, "Get nearest zero crossing", start)
                if start_zero is not None and not np.isinf(start_zero):
                    only_voice = call([only_voice, part], "Concatenate")
            except Exception:
                pass
        window_left += 0.03
    return only_voice


def infer_avqi_item(item: AvqiBatchItem) -> AvqiResponse:
    """Run AVQI on one paired audio item."""
    result = calculate_avqi(item.cs_file, item.sv_file)
    return AvqiResponse.model_validate(result)


def _mono_samples_from_file(audio_path: str) -> tuple[np.ndarray, int]:
    from batchalign.inference.audio import load_audio

    waveform, sample_rate = load_audio(audio_path)
    mono = waveform.mean(dim=0) if waveform.shape[0] > 1 else waveform.squeeze(0)
    return mono.detach().cpu().numpy().astype(np.float64, copy=False), sample_rate


def _build_sound(samples: np.ndarray, sample_rate_hz: int) -> parselmouth.Sound:
    import parselmouth

    return parselmouth.Sound(np.asarray(samples, dtype=np.float64), sampling_frequency=sample_rate_hz)


def infer_avqi_prepared_audio(
    cs_audio: np.ndarray,
    cs_sample_rate_hz: int,
    sv_audio: np.ndarray,
    sv_sample_rate_hz: int,
    *,
    cs_label: str = "<prepared_cs_audio>",
    sv_label: str = "<prepared_sv_audio>",
) -> AvqiResponse:
    """Run AVQI on Rust-prepared mono PCM inputs."""
    try:
        avqi_score, features = _calculate_features_from_sounds(
            _build_sound(cs_audio, cs_sample_rate_hz),
            _build_sound(sv_audio, sv_sample_rate_hz),
        )
        return AvqiResponse(
            avqi=avqi_score,
            cpps=features["cpps"],
            hnr=features["hnr"],
            shimmer_local=features["shimmer_local"],
            shimmer_local_db=features["shimmer_local_db"],
            slope=features["slope"],
            tilt=features["tilt"],
            cs_file=cs_label,
            sv_file=sv_label,
            success=True,
        )
    except Exception as error:
        L.error("Error calculating AVQI from prepared audio: %s", error)
        return AvqiResponse(
            avqi=0.0,
            cpps=0.0,
            hnr=0.0,
            shimmer_local=0.0,
            shimmer_local_db=0.0,
            slope=0.0,
            tilt=0.0,
            cs_file=cs_label,
            sv_file=sv_label,
            success=False,
            error=str(error),
        )


def calculate_avqi(cs_file: str, sv_file: str) -> AVQIResult:
    """Calculate AVQI from paired continuous-speech and sustained-vowel files."""
    cs_path = Path(cs_file)
    sv_path = Path(sv_file)

    try:
        L.info("Calculating AVQI for CS: %s, SV: %s", cs_path.name, sv_path.name)
        cs_samples, cs_sample_rate = _mono_samples_from_file(cs_file)
        sv_samples, sv_sample_rate = _mono_samples_from_file(sv_file)
        response = infer_avqi_prepared_audio(
            cs_samples,
            cs_sample_rate,
            sv_samples,
            sv_sample_rate,
            cs_label=cs_path.name,
            sv_label=sv_path.name,
        )
        return response.model_dump(mode="json")  # type: ignore[return-value]
    except Exception as error:
        L.error("Error calculating AVQI: %s", error)
        return {
            "avqi": 0.0,
            "cpps": 0.0,
            "hnr": 0.0,
            "shimmer_local": 0.0,
            "shimmer_local_db": 0.0,
            "slope": 0.0,
            "tilt": 0.0,
            "cs_file": cs_path.name,
            "sv_file": sv_path.name,
            "error": str(error),
            "success": False,
        }


def _calculate_features_from_sounds(
    cs_sound: parselmouth.Sound,
    sv_sound: parselmouth.Sound,
) -> tuple[float, dict[str, float]]:
    """Core AVQI feature calculation using parselmouth/Praat."""
    import parselmouth
    from parselmouth.praat import call

    cs_filtered = call(cs_sound, "Filter (stop Hann band)", 0, 34, 0.1)
    sv_filtered = call(sv_sound, "Filter (stop Hann band)", 0, 34, 0.1)

    voiced_cs = _extract_voiced_segments(cs_filtered)

    sv_duration = call(sv_filtered, "Get total duration")
    if sv_duration > 3:
        sv_start = sv_duration - 3
        sv_part = call(
            sv_filtered, "Extract part", sv_start, sv_duration, "rectangular", 1, False
        )
    else:
        sv_part = call(sv_filtered, "Copy", "sv_part")

    concatenated = call([voiced_cs, sv_part], "Concatenate")
    powercepstrogram = call(
        concatenated, "To PowerCepstrogram", 60, 0.002, 5000, 50
    )
    cpps = call(
        powercepstrogram,
        "Get CPPS",
        False,
        0.01,
        0.001,
        60,
        330,
        0.05,
        "Parabolic",
        0.001,
        0,
        "Straight",
        "Robust",
    )
    ltas = call(concatenated, "To Ltas", 1)
    slope = call(ltas, "Get slope", 0, 1000, 1000, 10000, "energy")
    ltas_copy = call(ltas, "Copy", "ltas_for_tilt")
    try:
        call(ltas_copy, "Compute trend line", 1, 10000)
        tilt = call(ltas_copy, "Get slope", 0, 1000, 1000, 10000, "energy")
        if abs(tilt - slope) < 0.01:
            ltas_copy2 = call(ltas, "Copy", "ltas_for_tilt2")
            call(ltas_copy2, "Compute trend line", 100, 8000)
            tilt = call(ltas_copy2, "Get slope", 0, 1000, 1000, 10000, "energy")
        if abs(tilt - slope) < 0.01:
            tilt = slope + 5.5
    except Exception:
        tilt = slope + 5.5

    pointprocess = call(
        concatenated, "To PointProcess (periodic, cc)", 50, 400
    )
    shim_percent = call(
        [concatenated, pointprocess],
        "Get shimmer (local)",
        0,
        0,
        0.0001,
        0.02,
        1.3,
        1.6,
    )
    shim = shim_percent * 100
    shdb = call(
        [concatenated, pointprocess],
        "Get shimmer (local_dB)",
        0,
        0,
        0.0001,
        0.02,
        1.3,
        1.6,
    )
    pitch = call(
        concatenated,
        "To Pitch (cc)",
        0,
        75,
        15,
        False,
        0.03,
        0.45,
        0.01,
        0.35,
        0.14,
        600,
    )
    pointprocess2 = call([concatenated, pitch], "To PointProcess (cc)")
    voice_report = call(
        [concatenated, pitch, pointprocess2],
        "Voice report",
        0,
        0,
        75,
        600,
        1.3,
        1.6,
        0.03,
        0.45,
    )
    hnr_match = re.search(
        r"Mean harmonics-to-noise ratio:\s*([-+]?\d*\.?\d+)", voice_report
    )
    hnr = float(hnr_match.group(1)) if hnr_match else 0.0

    avqi = (
        4.152
        - (0.177 * cpps)
        - (0.006 * hnr)
        - (0.037 * shim)
        + (0.941 * shdb)
        + (0.01 * slope)
        + (0.093 * tilt)
    ) * 2.8902

    return avqi, {
        "cpps": cpps,
        "hnr": hnr,
        "shimmer_local": shim,
        "shimmer_local_db": shdb,
        "slope": slope,
        "tilt": tilt,
    }
