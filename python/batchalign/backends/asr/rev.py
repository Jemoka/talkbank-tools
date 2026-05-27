"""RevAI: atomic-call ASR + Speaker diarization.

Rev.AI is the canonical "atomic-call" backend — one submission yields
both the ASR transcript with word timestamps and speaker diarization.
The engine batches it at :meth:`BatchPolicy.one` (one job per audio);
the backend dedupes by ``source_id`` and projects the single response
into either :class:`AsrOutput` or :class:`SpeakerOutput` depending on
which input variant arrived.

API key resolution (see :mod:`batchalign.config`):

1. ``BATCHALIGN_REVAI_KEY`` env var
2. ``~/.batchalign.ini`` ``[asr] engine.rev.key``
"""

from __future__ import annotations

import io
import time
import wave
from typing import Any

from batchalign.backends.base import ASR, Speaker, BatchPolicy
from batchalign import config


class RevAI(ASR, Speaker):
    """Rev.AI cloud ASR + diarization, atomic-call."""

    def __init__(
        self,
        api_key: str | None = None,
        *,
        language: str | None = None,
        poll_interval_s: float = 5.0,
        timeout_s: float = 3600.0,
    ) -> None:
        key = api_key if api_key is not None else config.get_api_key("revai")
        if not key:
            self._client = None
        else:
            from rev_ai import apiclient  # type: ignore[import-not-found]

            self._client = apiclient.RevAiAPIClient(key)
        self._poll = poll_interval_s
        self._timeout = timeout_s
        # Rev.AI language code (BA2 maps ISO-639-3 → -1, zho→cmn). `None`/"auto"
        # lets Rev pick its default (English).
        self._language = _rev_lang(language)
        self._policy = BatchPolicy.one()

    @property
    def name(self) -> str:
        return "revai:async-v2"

    @property
    def batch_policy(self) -> BatchPolicy:
        return self._policy

    def call(self, batch: list[Any]) -> list[Any]:
        from batchalign._core.proto import (
            AsrInput,
            AsrOutput,
            AsrSegment,
            AsrWord,
            SpeakerInput,
            SpeakerOutput,
            Diarization,
            DiarizationSegment,
        )

        if self._client is None:
            raise RuntimeError(
                "RevAI backend has no API key configured. Set "
                "BATCHALIGN_REVAI_KEY or add `[asr] engine.rev.key = ...` "
                "to ~/.batchalign.ini."
            )

        # Submit each unique source_id exactly once; project results.
        responses: dict[str, dict[str, Any]] = {}
        for item in batch:
            if item.source_id in responses:
                continue
            responses[item.source_id] = self._submit_and_wait(item.audio)

        outputs: list[Any] = []
        for item in batch:
            resp = responses[item.source_id]
            if isinstance(item, AsrInput):
                outputs.append(
                    AsrOutput(
                        source_id=item.source_id,
                        segments=_segments_from_rev(resp, AsrSegment, AsrWord),
                    )
                )
            elif isinstance(item, SpeakerInput):
                outputs.append(
                    SpeakerOutput(
                        source_id=item.source_id,
                        diarization=Diarization(
                            segments=_diar_from_rev(resp, DiarizationSegment),
                        ),
                    )
                )
            else:
                raise TypeError(
                    f"RevAI does not handle input type: {type(item).__name__}"
                )
        return outputs

    # ----- HTTP submission ----------------------------------------------

    def _submit_and_wait(self, audio: Any) -> dict[str, Any]:
        """Upload PCM-as-WAV, poll until ``transcribed``, return parsed JSON."""
        from rev_ai import JobStatus  # type: ignore[import-not-found]

        wav_bytes = _pcm_to_wav_bytes(audio)
        submit_kwargs: dict[str, Any] = {}
        if self._language:
            submit_kwargs["language"] = self._language
        job = self._client.submit_job_local_file(
            filename="audio.wav",
            metadata="batchalign",
            source_config=None,
            content_type="audio/wav",
            audio_data=io.BytesIO(wav_bytes),
            **submit_kwargs,
        ) if hasattr(self._client, "submit_job_local_file") else self._client.submit_job_local_file_buffer(
            audio_data=io.BytesIO(wav_bytes),
            filename="audio.wav",
        )

        deadline = time.monotonic() + self._timeout
        while True:
            details = self._client.get_job_details(job.id)
            status = getattr(details, "status", None)
            if status == JobStatus.TRANSCRIBED:
                break
            if status == JobStatus.FAILED:
                raise RuntimeError(f"Rev.AI job {job.id} failed: {details!r}")
            if time.monotonic() > deadline:
                raise TimeoutError(f"Rev.AI job {job.id} did not finish in {self._timeout}s")
            time.sleep(self._poll)
        return self._client.get_transcript_json(job.id)


def _rev_lang(code: str | None) -> str | None:
    """Map a CHAT/ISO language code to Rev.AI's code (BA2's mapping).

    `None`/"auto" → `None` (Rev defaults to English). ISO-639-3 is mapped to
    -1 via pycountry; Mandarin (`zho`/`zh`) becomes `cmn`, which is what Rev
    expects (BA2 `asr/rev.py`).
    """
    if not code or code == "auto":
        return None
    c = code.strip().lower()
    if c in ("zho", "zh", "cmn", "zh-hans", "zh-hant"):
        return "cmn"
    if len(c) <= 2:
        return c
    try:
        import pycountry  # type: ignore[import-not-found]

        lang = pycountry.languages.get(alpha_3=c)
        if lang is not None and getattr(lang, "alpha_2", None):
            return lang.alpha_2
    except Exception:
        pass
    return c


# ---------------------------------------------------------------------------
# Pure functions — easy to unit-test against a recorded Rev.AI JSON response.
# ---------------------------------------------------------------------------


def _segments_from_rev(
    resp: dict[str, Any], AsrSegment: type, AsrWord: type
) -> list[Any]:
    """Project a Rev.AI transcript JSON into a list of :class:`AsrSegment`.

    Rev's JSON is ``{"monologues": [{"speaker": int, "elements":
    [{"type": "text", "value": "...", "ts": float, "end_ts": float,
    "confidence": float}, ...]}, ...]}``.
    """
    segments: list[Any] = []
    for mono in resp.get("monologues", []):
        words: list[Any] = []
        for el in mono.get("elements", []):
            if el.get("type") != "text":
                continue
            words.append(
                AsrWord(
                    text=el.get("value", "").strip(),
                    start_ms=int((el.get("ts") or 0.0) * 1000),
                    end_ms=int((el.get("end_ts") or 0.0) * 1000),
                    confidence=el.get("confidence"),
                )
            )
        if not words:
            continue
        text = " ".join(w.text for w in words if w.text)
        segments.append(
            AsrSegment(
                start_ms=words[0].start_ms,
                end_ms=words[-1].end_ms,
                text=text,
                speaker=str(mono.get("speaker")) if mono.get("speaker") is not None else None,
                words=words,
            )
        )
    return segments


def _diar_from_rev(
    resp: dict[str, Any], DiarizationSegment: type
) -> list[Any]:
    """Project Rev.AI monologues into diarization spans."""
    segs: list[Any] = []
    for mono in resp.get("monologues", []):
        elements = [e for e in mono.get("elements", []) if e.get("type") == "text"]
        if not elements:
            continue
        start_ms = int((elements[0].get("ts") or 0.0) * 1000)
        end_ms = int((elements[-1].get("end_ts") or 0.0) * 1000)
        segs.append(
            DiarizationSegment(
                start_ms=start_ms,
                end_ms=end_ms,
                speaker=str(mono.get("speaker", "0")),
            )
        )
    return segs


def _pcm_to_wav_bytes(audio: Any) -> bytes:
    """Encode PCM-float32 ``audio`` as a 16-bit mono WAV byte string."""
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


__all__ = ["RevAI"]
