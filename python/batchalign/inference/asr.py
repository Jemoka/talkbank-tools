"""ASR inference: audio -> raw tokens with timestamps.

Pure inference — returns raw engine-shaped ASR payloads for Rust post-processing.
No CHAT assembly, no number expansion, no retokenization.
"""

from __future__ import annotations

import logging
from typing import TYPE_CHECKING, Literal

import numpy as np
from pydantic import BaseModel, Field

logger = logging.getLogger(__name__)
import pycountry

from batchalign.inference._domain_types import (
    AudioPath,
    ConfidenceScore,
    LanguageCode,
    NumSpeakers,
    RevAiJobId,
    SampleRate,
    SpeakerId,
    TimestampSeconds,
)

if TYPE_CHECKING:
    from batchalign.inference.types import WhisperASRHandle

# ---------------------------------------------------------------------------
# Pydantic models
# ---------------------------------------------------------------------------


class AsrBatchItem(BaseModel):
    """A single ASR inference request."""

    audio_path: AudioPath
    lang: LanguageCode = "eng"
    num_speakers: NumSpeakers = 1
    rev_job_id: RevAiJobId | None = None


class AsrElement(BaseModel):
    """One raw ASR element in a speaker monologue payload."""

    value: str
    ts: TimestampSeconds | None = None
    end_ts: TimestampSeconds | None = None
    type: str = "text"
    confidence: ConfidenceScore | None = None


class AsrMonologue(BaseModel):
    """One speaker-attributed ASR span returned by a provider adapter."""

    speaker: int | SpeakerId = 0
    elements: list[AsrElement] = Field(default_factory=list)


class MonologueAsrResponse(BaseModel):
    """Tagged raw ASR payload built from speaker monologues."""

    kind: Literal["monologues"] = "monologues"
    lang: LanguageCode = "eng"
    monologues: list[AsrMonologue] = Field(default_factory=list)


# ---------------------------------------------------------------------------
# Whisper pipeline output boundary models
# ---------------------------------------------------------------------------


class WhisperChunk(BaseModel):
    """One chunk from HuggingFace Whisper pipeline output."""

    text: str = ""
    timestamp: tuple[float | None, float | None] | list[float | None] = (None, None)


class WhisperChunksAsrResponse(BaseModel):
    """Tagged raw ASR payload for the local Whisper pipeline output."""

    kind: Literal["whisper_chunks"] = "whisper_chunks"
    lang: LanguageCode = "eng"
    text: str = ""
    chunks: list[WhisperChunk] = Field(default_factory=list)


def iso3_to_language_name(iso3: LanguageCode) -> str:
    """Convert ISO-639-3 language code to a Whisper language name.

    Raises ``ValueError`` if the code is not recognized by pycountry.
    Previously this silently fell back to ``"english"``, which caused
    wrong-language transcription with no warning — a regression from
    batchalign2 which used the same pycountry lookup but in a context
    where unrecognized codes would surface earlier.
    """
    special: dict[str, str] = {"yue": "Cantonese", "cmn": "chinese", "auto": "auto"}
    if iso3 in special:
        return special[iso3]
    lang_obj = pycountry.languages.get(alpha_3=iso3)
    if lang_obj is not None:
        return str(lang_obj.name).lower()
    raise ValueError(
        f"Unrecognized ISO 639-3 language code '{iso3}' — pycountry has no "
        f"entry for this code. Whisper cannot determine the target language. "
        f"Check that the --lang value is a valid ISO 639-3 code."
    )


# ---------------------------------------------------------------------------
# Whisper ASR load/infer (replaces WhisperASRModel class)
# ---------------------------------------------------------------------------


def load_whisper_asr(
    model: str = "openai/whisper-large-v3",
    base: str = "openai/whisper-large-v3",
    language: str = "english",
    target_sample_rate: SampleRate = 16000,
    *,
    device_policy=None,
) -> WhisperASRHandle:
    """Load a Whisper ASR pipeline. Returns a typed handle."""
    from batchalign.inference.types import WhisperASRHandle
    import torch
    from transformers import (
        GenerationConfig,
        WhisperProcessor,
        WhisperTokenizer,
        pipeline,
    )

    from batchalign.inference.audio import bind_whisper_token_timestamp_extractor

    from batchalign.device import resolve_inference_device
    from batchalign.worker._progress import (
        HF_ARTIFACTS_WHISPER,
        emit_hf_download_if_missing,
    )

    device = resolve_inference_device(device_policy)

    # Surface a download notification if the user is about to wait for a
    # multi-GB Whisper download. ``base`` and ``model`` may be the same
    # repo; we probe both because either path could trigger a download
    # depending on which file was previously cached. Probe the full
    # Whisper artifact set (config + generation_config + tokenizer +
    # tokenizer_config + preprocessor_config) so a partial-cache state
    # doesn't bypass the notification.
    emit_hf_download_if_missing(base, kind="ASR", artifacts=HF_ARTIFACTS_WHISPER)
    if model != base:
        emit_hf_download_if_missing(
            model, kind="ASR", artifacts=HF_ARTIFACTS_WHISPER
        )

    config = GenerationConfig.from_pretrained(base)
    config.no_repeat_ngram_size = 4
    config.use_cache = True

    if language == "Cantonese":
        config.no_timestamps_token_id = 50363
        config.alignment_heads = [
            [5, 3], [5, 9], [8, 0], [8, 4], [8, 8],
            [9, 0], [9, 7], [9, 9], [10, 5],
        ]

    asr_dtype = torch.float16 if device.type == "cuda" else torch.float32

    pipe = pipeline(
        "automatic-speech-recognition",
        model=model,
        tokenizer=WhisperTokenizer.from_pretrained(base),
        chunk_length_s=25,
        stride_length_s=3,
        device=device,
        torch_dtype=asr_dtype,
        return_timestamps=True,
    )
    bind_whisper_token_timestamp_extractor(pipe.model)
    pipe.model.eval()
    WhisperProcessor.from_pretrained(base)

    return WhisperASRHandle(
        pipe=pipe,
        config=config,
        lang=language,
        sample_rate=target_sample_rate,
    )


def _infer_whisper(
    model: WhisperASRHandle,
    item: AsrBatchItem,
) -> WhisperChunksAsrResponse:
    """Run local Whisper inference and return the pipeline's chunk payload.

    Calls the HuggingFace pipeline with the source path directly and extracts
    the raw chunk payload. That keeps the Python adapter closer to a pure model
    host: it does not decode or resample audio itself before inference.

    No turn assembly, no speaker interleaving, no punctuation splitting. Rust
    still owns all shared postprocessing after deserializing this tagged
    payload.
    """
    gen_kwargs = model.gen_kwargs(model.lang)

    raw = model(item.audio_path, batch_size=1, generate_kwargs=gen_kwargs)

    # Parse pipeline output at the boundary into typed model
    output = WhisperChunksAsrResponse.model_validate(
        {
            "kind": "whisper_chunks",
            "lang": item.lang,
            "text": raw.get("text", "") if isinstance(raw, dict) else "",
            "chunks": raw.get("chunks", []) if isinstance(raw, dict) else [],
        }
    )

    return output


def infer_whisper_prepared_audio(
    model: WhisperASRHandle,
    audio: np.ndarray,
    lang: LanguageCode,
) -> WhisperChunkResultPayloadV2:
    """Run local Whisper on Rust-prepared mono audio.

    This is the local-model V2 boundary for ASR. Rust owns media decoding and
    audio preparation, while Python just feeds the prepared waveform into the
    HuggingFace runtime and returns the raw chunk payload.
    """
    from batchalign.worker._types_v2 import WhisperChunkResultPayloadV2, WhisperChunkSpanV2

    gen_kwargs = model.gen_kwargs(iso3_to_language_name(lang))
    raw = model(
        {"raw": np.asarray(audio, dtype=np.float32), "sampling_rate": model.sample_rate},
        batch_size=1,
        generate_kwargs=gen_kwargs,
    )

    chunks = raw.get("chunks", []) if isinstance(raw, dict) else []
    clamped_chunks: list[WhisperChunkSpanV2] = []
    for chunk in chunks:
        start_s: float = (chunk.get("timestamp") or [None, None])[0] or 0.0
        end_s: float = (chunk.get("timestamp") or [None, None])[1] or 0.0
        if end_s < start_s:
            logger.warning(
                "Whisper chunk has inverted timestamps (start=%.1f > end=%.1f), swapping",
                start_s,
                end_s,
            )
            start_s, end_s = end_s, start_s
        clamped_chunks.append(
            WhisperChunkSpanV2(
                text=str(chunk.get("text", "")),
                start_s=start_s,
                end_s=end_s,
            )
        )
    return WhisperChunkResultPayloadV2(
        lang=lang,
        text=raw.get("text", "") if isinstance(raw, dict) else "",
        chunks=clamped_chunks,
    )
