"""OpenAI Whisper (local ``openai-whisper`` package) ASR backend.

Distinct from :class:`WhisperBackend` (HF Transformers) and
:class:`VllmAsrBackend` (HTTP). This backend uses the original
``openai/whisper`` PyPI package which supports the ``turbo`` model and
emits word-level timestamps via ``word_timestamps=True``.
"""

from __future__ import annotations

import os
import tempfile
from typing import Any

from batchalign.backends.base import ASR, BatchPolicy


class OpenAIWhisperBackend(ASR):
    """Local Whisper via the ``openai-whisper`` PyPI package.

    Default model is ``"turbo"`` (the latest speed/quality preset
    shipped with openai-whisper >= 20240930). Word-level timestamps
    come from ``model.transcribe(..., word_timestamps=True)``.
    """

    def __init__(
        self,
        model: str = "turbo",
        *,
        language: str | None = None,
        device: str | None = None,
        batch_size: int = 1,
        batch_window_ms: int = 0,
    ) -> None:
        import whisper  # type: ignore[import-not-found]

        self._model = whisper.load_model(model, device=device)
        self._model_id = model
        # Fallback language for when the runner ships `Auto` (it always does).
        self._language = None if language in (None, "auto") else language
        self._policy = BatchPolicy(max_size=batch_size, window_ms=batch_window_ms)

    @property
    def name(self) -> str:
        # v2: one AsrSegment per Whisper segment (was a single joined blob).
        return f"openai-whisper:{self._model_id}:v2"

    @property
    def batch_policy(self) -> BatchPolicy:
        return self._policy

    def call(self, batch: list[Any]) -> list[Any]:
        from batchalign._core.proto import AsrInput, AsrOutput, AsrSegment, AsrWord
        from batchalign.backends.asr.vllm import pcm_to_wav_bytes

        outputs: list[Any] = []
        for item in batch:
            if not isinstance(item, AsrInput):
                raise TypeError(
                    f"OpenAIWhisperBackend does not handle: {type(item).__name__}"
                )
            wav_bytes = pcm_to_wav_bytes(item.audio)
            # openai-whisper expects a file path or numpy array; write a
            # temp WAV for path-based load (handles arbitrary sample rates).
            with tempfile.NamedTemporaryFile(suffix=".wav", delete=False) as tmp:
                tmp.write(wav_bytes)
                tmp_path = tmp.name
            try:
                kwargs: dict[str, Any] = {"word_timestamps": True}
                lang = (
                    item.language.value
                    if item.language.kind == "code" and item.language.value
                    else self._language
                )
                if lang:
                    kwargs["language"] = lang
                result = self._model.transcribe(tmp_path, **kwargs)
            finally:
                try:
                    os.unlink(tmp_path)
                except OSError:
                    pass

            # BA2's OAIWhisperEngine builds ONE turn per Whisper segment
            # (`for i in res["segments"]: turns.append(...)`), so each Whisper
            # segment becomes its own utterance before the CHATUtterance BERT
            # stage. Mirror that: one AsrSegment per Whisper segment, with
            # word-attached punctuation stripped (process_generation drops it)
            # so each parses as one CHAT utterance.
            segments_out: list[Any] = []
            for seg in result.get("segments", []):
                text = (seg.get("text") or "").strip()
                for _p in (".", ",", "?", "!", ";", ":", '"', "„", "“", "”", "‡", "«", "»"):
                    text = text.replace(_p, " ")
                text = " ".join(text.split())
                if not text:
                    continue
                segments_out.append(
                    AsrSegment(
                        start_ms=int((seg.get("start") or 0.0) * 1000),
                        end_ms=int((seg.get("end") or 0.0) * 1000),
                        text=text,
                        speaker=None,
                        words=[],
                    )
                )
            outputs.append(AsrOutput(source_id=item.source_id, segments=segments_out))
        return outputs


__all__ = ["OpenAIWhisperBackend"]
