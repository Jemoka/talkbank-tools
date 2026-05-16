"""ASR via a vLLM server exposing Whisper through the OpenAI audio API.

vLLM serves an OpenAI-compatible HTTP endpoint, so we talk to it via
the ``openai`` SDK. The user is responsible for starting the server
with a Whisper-class model loaded — see https://docs.vllm.ai/.
"""

from __future__ import annotations

import io
import wave
from typing import Any

from batchalign.backends.base import ASR, BatchPolicy


class VllmAsrBackend(ASR):
    """ASR via a vLLM server exposing Whisper through the OpenAI audio API."""

    def __init__(
        self,
        *,
        model: str = "openai/whisper-large-v3",
        base_url: str = "http://localhost:8000/v1",
        api_key: str = "EMPTY",
        batch_size: int = 32,
        batch_window_ms: int = 50,
    ) -> None:
        from openai import OpenAI  # type: ignore[import-not-found]

        self._client = OpenAI(base_url=base_url, api_key=api_key)
        self._model = model
        self._policy = BatchPolicy(max_size=batch_size, window_ms=batch_window_ms)

    @property
    def name(self) -> str:
        return f"vllm-asr:{self._model}"

    @property
    def batch_policy(self) -> BatchPolicy:
        return self._policy

    def call(self, batch: list[Any]) -> list[Any]:
        from batchalign._core.proto import AsrInput, AsrOutput, AsrSegment, AsrWord

        outputs: list[Any] = []
        for item in batch:
            if not isinstance(item, AsrInput):
                raise TypeError(
                    f"VllmAsrBackend does not handle: {type(item).__name__}"
                )
            wav = pcm_to_wav_bytes(item.audio)
            kwargs: dict[str, Any] = {
                "model": self._model,
                "file": ("audio.wav", wav, "audio/wav"),
                "response_format": "verbose_json",
                "timestamp_granularities": ["word"],
            }
            if item.language.kind == "code" and item.language.value:
                kwargs["language"] = item.language.value
            resp = self._client.audio.transcriptions.create(**kwargs)
            outputs.append(asr_from_openai(resp, item.source_id, AsrOutput, AsrSegment, AsrWord))
        return outputs


# ---------------------------------------------------------------------------
# Shared helpers (also used by openai_whisper backend).
# ---------------------------------------------------------------------------


def pcm_to_wav_bytes(audio: Any) -> bytes:
    """Encode PCM-float32 audio as a 16-bit mono WAV byte string."""
    import numpy as np  # type: ignore[import-not-found]

    arr = np.frombuffer(audio.pcm_f32le, dtype=np.float32)
    pcm16 = (np.clip(arr, -1.0, 1.0) * 32767.0).astype(np.int16)
    buf = io.BytesIO()
    with wave.open(buf, "wb") as wf:
        wf.setnchannels(1)
        wf.setsampwidth(2)
        wf.setframerate(int(audio.sample_rate))
        wf.writeframes(pcm16.tobytes())
    return buf.getvalue()


def asr_from_openai(
    resp: Any,
    source_id: str,
    AsrOutput: type,
    AsrSegment: type,
    AsrWord: type,
) -> Any:
    """Build an :class:`AsrOutput` from an OpenAI ``verbose_json`` response."""
    payload = resp.model_dump() if hasattr(resp, "model_dump") else dict(resp)
    words_raw = payload.get("words") or []
    words = [
        AsrWord(
            text=w.get("word") or w.get("text") or "",
            start_ms=int((w.get("start") or 0.0) * 1000),
            end_ms=int((w.get("end") or 0.0) * 1000),
            confidence=None,
        )
        for w in words_raw
    ]
    text = (payload.get("text") or " ".join(w.text for w in words)).strip()
    if not words:
        return AsrOutput(source_id=source_id, segments=[])
    seg = AsrSegment(
        start_ms=words[0].start_ms,
        end_ms=words[-1].end_ms,
        text=text,
        speaker=None,
        words=words,
    )
    return AsrOutput(source_id=source_id, segments=[seg])


__all__ = ["VllmAsrBackend", "pcm_to_wav_bytes", "asr_from_openai"]
