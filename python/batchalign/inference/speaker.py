"""Speaker diarization inference: prepared audio -> raw speaker segments.

This module is the thinnest Python seam for diarization runtimes. The ownership
split is:

- Rust chooses when speaker diarization should run and which backend to use.
- Rust prepares audio and sends it via worker protocol V2.
- Python opens the model runtime and returns raw timestamped speaker segments.
- Rust owns any higher-level document semantics or workflow policy above this
  boundary.

Production entry point: ``infer_speaker_prepared_audio()`` receives
Rust-prepared mono PCM and dispatches to the appropriate backend.
"""

from __future__ import annotations

from contextlib import contextmanager
import logging
import wave
from pathlib import Path
from typing import TYPE_CHECKING

import numpy as np
from pydantic import BaseModel

from batchalign.inference._domain_types import (
    AudioPath,
    NumSpeakers,
    SpeakerId,
    TimestampMs,
)

if TYPE_CHECKING:
    import torch

L = logging.getLogger("batchalign.worker")

_PYANNOTE_PIPELINE: object | None = None


def infer_speaker_prepared_audio(
    audio: np.ndarray,
    sample_rate_hz: int,
    *,
    num_speakers: NumSpeakers = 2,
    engine: str = "pyannote",
    device_policy=None,
) -> "SpeakerResponse":
    """Run one prepared-audio diarization item through the requested backend."""

    if engine == "nemo":
        segments = infer_nemo_speaker_prepared_audio(
            audio,
            sample_rate_hz,
            num_speakers,
            device_policy=device_policy,
        )
    else:
        segments = infer_pyannote_speaker_prepared_audio(audio, sample_rate_hz, num_speakers)
    return SpeakerResponse(segments=segments)


# ---------------------------------------------------------------------------
# Pydantic models
# ---------------------------------------------------------------------------


class SpeakerSegment(BaseModel):
    """A single speaker segment."""

    start_ms: TimestampMs
    end_ms: TimestampMs
    speaker: SpeakerId


class SpeakerResponse(BaseModel):
    """Speaker diarization output."""

    segments: list[SpeakerSegment]


# ---------------------------------------------------------------------------
# RTTM parsing
# ---------------------------------------------------------------------------

# RTTM (Rich Transcription Time Marked) field indices.
# Standard format: TYPE FILE CHANNEL TBEG TDUR <NA> <NA> SPEAKER <NA> <NA>
_RTTM_FIELD_COUNT = 10
_RTTM_TBEG = 3
_RTTM_TDUR = 4
_RTTM_SPEAKER = 7


def _parse_rttm_line(line: str) -> SpeakerSegment:
    """Parse a single RTTM line into a SpeakerSegment."""
    fields = line.split()
    if len(fields) < _RTTM_FIELD_COUNT:
        raise ValueError(
            f"RTTM line has {len(fields)} fields, expected {_RTTM_FIELD_COUNT}: {line!r}"
        )
    start_ms = int(float(fields[_RTTM_TBEG]) * 1000)
    dur_ms = int(float(fields[_RTTM_TDUR]) * 1000)
    speaker_label = fields[_RTTM_SPEAKER]
    # Extract trailing numeric suffix: "speaker_0" -> "0"
    speaker_id = speaker_label.split("_")[-1] if "_" in speaker_label else speaker_label
    return SpeakerSegment(
        start_ms=start_ms,
        end_ms=start_ms + dur_ms,
        speaker=f"SPEAKER_{speaker_id}",
    )


# ---------------------------------------------------------------------------
# NeMo speaker diarization load/infer
# ---------------------------------------------------------------------------


def _conv_scale_weights(
    self: torch.nn.Module,  # NeMo MSDD module (dynamic attributes)
    ms_avg_embs_perm: torch.Tensor,
    ms_emb_seq_single: torch.Tensor,
) -> torch.Tensor:
    """CNN scale weight computation for NeMo MSDD.

    This is a monkey-patch for NeMo's MSDD_module.conv_scale_weights.
    The ``self`` parameter is an MSDD_module instance with dynamic attributes
    (conv, conv_bn, conv_repeat, batch_size, etc.) that mypy cannot see.
    """
    import torch  # noqa: F811
    import torch.nn.functional as F

    ms_cnn_input_seq = torch.cat([ms_avg_embs_perm, ms_emb_seq_single], dim=2)
    ms_cnn_input_seq = ms_cnn_input_seq.unsqueeze(2).flatten(0, 1)

    # All attribute accesses below: nn.Module.__getattr__ returns
    # Tensor | Module, but at runtime these are MSDD dynamic attributes.
    conv_out = self.conv_forward(
        ms_cnn_input_seq, conv_module=self.conv[0], bn_module=self.conv_bn[0], first_layer=True
    )
    for conv_idx in range(1, self.conv_repeat + 1):
        conv_out = self.conv_forward(
            conv_input=conv_out,
            conv_module=self.conv[conv_idx],
            bn_module=self.conv_bn[conv_idx],
            first_layer=False,
        )

    lin_input_seq = conv_out.reshape(self.batch_size, self.length, self.cnn_output_ch * self.emb_dim)
    hidden_seq = self.conv_to_linear(lin_input_seq)
    hidden_seq = self.dropout(F.leaky_relu(hidden_seq))
    scale_weights = self.softmax(self.linear_to_weights(hidden_seq))
    scale_weights = scale_weights.unsqueeze(3).expand(-1, -1, -1, self.num_spks)
    return scale_weights


def _resolve_speaker_config() -> str:
    """Return path to the NeMo speaker diarization config.yaml."""
    import os
    return os.path.join(os.path.dirname(__file__), "speaker_config.yaml")


@contextmanager
def _temporary_conv_scale_weights_override(msdd_module: object):
    """Apply the NeMo MSDD override only for one narrow execution window."""

    had_original = hasattr(msdd_module, "conv_scale_weights")
    original = getattr(msdd_module, "conv_scale_weights", None)
    setattr(msdd_module, "conv_scale_weights", _conv_scale_weights)
    try:
        yield
    finally:
        if had_original:
            setattr(msdd_module, "conv_scale_weights", original)
        else:
            delattr(msdd_module, "conv_scale_weights")


def _device_for_speaker_runtime(device_policy=None) -> str:
    """Resolve the concrete runtime device for diarization backends.

    MPS is explicitly excluded: Pyannote produces wrong timestamps on MPS
    (pyannote/pyannote-audio#1337, closed wontfix) and NeMo is CUDA-only.
    Delegates to the shared ``resolve_inference_device`` which encodes the
    CUDA > CPU selection with the MPS permanent exclusion.
    """
    from batchalign.device import resolve_inference_device

    return resolve_inference_device(device_policy).type


def _write_prepared_audio_wav(
    audio: np.ndarray,
    sample_rate_hz: int,
    output_path: str,
) -> None:
    """Write mono prepared PCM to a narrow worker-local WAV artifact."""

    mono = np.asarray(audio, dtype=np.float32)
    clipped = np.clip(mono, -1.0, 1.0)
    pcm16 = (clipped * np.iinfo(np.int16).max).astype("<i2")
    with wave.open(output_path, "wb") as handle:
        handle.setnchannels(1)
        handle.setsampwidth(2)
        handle.setframerate(sample_rate_hz)
        handle.writeframes(pcm16.tobytes())


def _infer_nemo_speaker_from_audio_file(
    audio_path: str,
    num_speakers: NumSpeakers,
    *,
    device_policy=None,
) -> list[SpeakerSegment]:
    """Run NeMo diarization from a concrete audio file path."""
    import copy
    import json
    import os
    import tempfile

    from omegaconf import OmegaConf
    from nemo.collections.asr.models.msdd_models import NeuralDiarizer
    from nemo.collections.asr.modules.msdd_diarizer import MSDD_module
    from pydub import AudioSegment

    base_config = OmegaConf.load(_resolve_speaker_config())
    config = copy.deepcopy(base_config)

    with tempfile.TemporaryDirectory() as workdir:
        sound = AudioSegment.from_file(audio_path).set_channels(1)
        mono_path = os.path.join(workdir, "mono_file.wav")
        sound.export(mono_path, format="wav")

        meta = {
            "audio_filepath": mono_path,
            "offset": 0,
            "duration": None,
            "label": "infer",
            "text": "-",
            "rttm_filepath": None,
            "uem_filepath": None,
            "num_speakers": num_speakers,
        }
        manifest_path = os.path.join(workdir, "input_manifest.json")
        with open(manifest_path, "w", encoding="utf-8") as fp:
            json.dump(meta, fp)
            fp.write("\n")
        config.diarizer.manifest_filepath = manifest_path
        config.diarizer.out_dir = workdir
        config.device = _device_for_speaker_runtime(device_policy)

        with _temporary_conv_scale_weights_override(MSDD_module):
            msdd_model = NeuralDiarizer(cfg=config)
            msdd_model.diarize()

        segments: list[SpeakerSegment] = []
        with open(os.path.join(workdir, "pred_rttms", "mono_file.rttm"), encoding="utf-8") as f:
            for line in f:
                segments.append(_parse_rttm_line(line))
        return segments


def infer_nemo_speaker_prepared_audio(
    audio: np.ndarray,
    sample_rate_hz: int,
    num_speakers: NumSpeakers = 2,
    *,
    device_policy=None,
) -> list[SpeakerSegment]:
    """Run NeMo diarization on Rust-prepared mono PCM audio."""
    import os
    import tempfile

    with tempfile.TemporaryDirectory() as workdir:
        wav_path = os.path.join(workdir, "prepared_audio.wav")
        _write_prepared_audio_wav(audio, sample_rate_hz, wav_path)
        return _infer_nemo_speaker_from_audio_file(
            wav_path,
            num_speakers,
            device_policy=device_policy,
        )


def infer_pyannote_speaker_prepared_audio(
    audio: np.ndarray,
    sample_rate_hz: int,
    num_speakers: NumSpeakers = 2,
) -> list[SpeakerSegment]:
    """Run Pyannote diarization on Rust-prepared mono PCM audio."""
    import torch

    waveform = torch.from_numpy(np.asarray(audio, dtype=np.float32)).unsqueeze(0)
    pipe = _get_pyannote_pipeline()
    result = pipe({"waveform": waveform, "sample_rate": sample_rate_hz}, num_speakers=num_speakers)

    segments: list[SpeakerSegment] = []
    for turn, speaker in _iter_pyannote_turns_and_speakers(result):
        segments.append(
            SpeakerSegment(
                start_ms=int(turn.start * 1000),
                end_ms=int(turn.end * 1000),
                speaker=f"SPEAKER_{speaker.split('_')[-1] if '_' in speaker else speaker}",
            )
        )
    return segments


def _iter_pyannote_turns_and_speakers(result: object):
    """Yield `(turn, speaker)` pairs across supported pyannote output shapes."""

    if hasattr(result, "itertracks"):
        for turn, _, speaker in result.itertracks(yield_label=True):
            yield turn, speaker
        return

    diarization = getattr(result, "speaker_diarization", None)
    if diarization is None:
        raise AttributeError(
            "pyannote diarization result exposes neither itertracks() nor speaker_diarization"
        )

    if hasattr(diarization, "itertracks"):
        for turn, _, speaker in diarization.itertracks(yield_label=True):
            yield turn, speaker
        return

    for turn, speaker in diarization:
        yield turn, speaker


def _get_pyannote_pipeline():
    """Return the shared Pyannote pipeline for this worker process.

    Raises ImportError if pyannote.audio is missing from the runtime. Speaker
    diarization is part of the standard batchalign3 install, so this indicates a
    broken environment rather than an optional feature tier.
    """
    global _PYANNOTE_PIPELINE

    if _PYANNOTE_PIPELINE is None:
        try:
            from pyannote.audio import Pipeline as PyannotePipeline
        except ImportError as exc:
            raise ImportError(
                "Speaker diarization requires pyannote.audio, which is not installed.\n"
                "Reinstall the standard batchalign3 package and confirm "
                "'import pyannote.audio' works in the worker Python runtime."
            ) from exc

        from batchalign.worker._progress import emit_hf_download_if_missing

        emit_hf_download_if_missing(
            "talkbank/dia-fork", kind="speaker diarization"
        )

        _PYANNOTE_PIPELINE = PyannotePipeline.from_pretrained("talkbank/dia-fork")
    return _PYANNOTE_PIPELINE
