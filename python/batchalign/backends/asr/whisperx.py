"""WhisperX backend: ASR + forced alignment in one library.

WhisperX (https://github.com/m-bain/whisperX) wraps faster-whisper for
transcription and a pretrained wav2vec2 / Charsiu model for
phoneme-level alignment. We register both task markers (`ASR`, `FA`)
so the kernel may route either input type here. Audio is chunked at
60s boundaries with offset accounting, mirroring BA2's
``batchalign2/batchalign/pipelines/asr/whisperx.py``.

All times are converted from seconds (whisperx native) to milliseconds.
"""

from __future__ import annotations

from typing import Any

from batchalign.backends.base import ASR, FA, BatchPolicy


_CHUNK_S = 60.0


class WhisperXBackend(ASR, FA):
    """Local WhisperX backend providing both ASR and FA.

    The same loaded model serves both tasks: ``call()`` dispatches on
    the input variant. Alignment uses ``whisperx.load_align_model`` for
    the requested language and ``whisperx.align`` against the segments
    (either freshly transcribed for ASR, or pre-existing for FA).
    """

    def __init__(
        self,
        model: str = "large-v2",
        *,
        language: str | None = None,
        device: str | None = None,
        compute_type: str = "float16",
        batch_size: int = 16,
        batch_window_ms: int = 50,
        chunk_length_s: float = _CHUNK_S,
    ) -> None:
        # Lazy import inside __init__ so importing this module is cheap.
        import whisperx  # type: ignore[import-not-found]

        if device is None:
            try:
                import torch  # type: ignore[import-not-found]

                device = "cuda" if torch.cuda.is_available() else "cpu"
            except ImportError:
                device = "cpu"
        if device == "cpu" and compute_type == "float16":
            compute_type = "int8"

        self._wx = whisperx
        self._device = device
        self._compute_type = compute_type
        self._model_id = model
        self._language = language
        self._chunk_s = chunk_length_s
        self._asr_model = whisperx.load_model(model, device, compute_type=compute_type)
        # Align models are language-specific; cache per language.
        self._align_cache: dict[str, tuple[Any, Any]] = {}
        self._policy = BatchPolicy(max_size=batch_size, window_ms=batch_window_ms)

    @property
    def name(self) -> str:
        return f"whisperx:{self._model_id}"

    @property
    def batch_policy(self) -> BatchPolicy:
        return self._policy

    def call(self, batch: list[Any]) -> list[Any]:
        from batchalign._core.proto import AsrInput, FaInput

        outputs: list[Any] = []
        for item in batch:
            if isinstance(item, AsrInput):
                outputs.append(self._run_asr(item))
            elif isinstance(item, FaInput):
                outputs.append(self._run_fa(item))
            else:
                raise TypeError(
                    f"WhisperXBackend does not handle: {type(item).__name__}"
                )
        return outputs

    # ----- internals -----------------------------------------------------

    def _audio_chunks(self, item: Any) -> list[tuple[float, Any]]:
        """Yield ``(offset_s, numpy_chunk)`` tuples at chunk boundaries."""
        import numpy as np  # type: ignore[import-not-found]

        wave = np.frombuffer(item.audio.pcm_f32le, dtype=np.float32)
        sr = int(item.audio.sample_rate)
        chunk_n = int(self._chunk_s * sr)
        if chunk_n <= 0 or len(wave) <= chunk_n:
            return [(0.0, wave)]
        chunks: list[tuple[float, Any]] = []
        for start in range(0, len(wave), chunk_n):
            offset_s = start / sr
            chunks.append((offset_s, wave[start : start + chunk_n]))
        return chunks

    def _ensure_align_model(self, language: str) -> tuple[Any, Any]:
        if language not in self._align_cache:
            self._align_cache[language] = self._wx.load_align_model(
                language_code=language, device=self._device
            )
        return self._align_cache[language]

    def _language_for(self, item: Any, fallback: str | None = None) -> str:
        if item.language.kind == "code" and item.language.value:
            return item.language.value
        return fallback or self._language or "en"

    def _run_asr(self, item: Any) -> Any:
        from batchalign._core.proto import AsrOutput, AsrSegment, AsrWord

        sr = int(item.audio.sample_rate)
        language = self._language_for(item)
        all_segments: list[Any] = []
        for offset_s, chunk in self._audio_chunks(item):
            kwargs: dict[str, Any] = {}
            if language:
                kwargs["language"] = language
            result = self._asr_model.transcribe(chunk, **kwargs)
            # whisperx may re-detect language; honour it if we had none.
            detected = result.get("language") or language
            align_model, meta = self._ensure_align_model(detected)
            aligned = self._wx.align(
                result["segments"], align_model, meta, chunk, self._device,
                return_char_alignments=False,
            )
            for seg in aligned.get("segments", []):
                words = [
                    AsrWord(
                        text=w.get("word") or "",
                        start_ms=int(((w.get("start") or 0.0) + offset_s) * 1000),
                        end_ms=int(((w.get("end") or 0.0) + offset_s) * 1000),
                        confidence=None,
                    )
                    for w in seg.get("words", [])
                    if w.get("start") is not None
                ]
                if not words:
                    continue
                all_segments.append(
                    AsrSegment(
                        start_ms=words[0].start_ms,
                        end_ms=words[-1].end_ms,
                        text=(seg.get("text") or "").strip(),
                        speaker=None,
                        words=words,
                    )
                )
        return AsrOutput(source_id=item.source_id, segments=all_segments)

    def _run_fa(self, item: Any) -> Any:
        """Forced alignment against pre-existing transcript segments."""
        from batchalign._core.proto import FaOutput, AsrSegment, AsrWord
        import numpy as np  # type: ignore[import-not-found]

        wave = np.frombuffer(item.audio.pcm_f32le, dtype=np.float32)
        language = self._language_for(item)
        align_model, meta = self._ensure_align_model(language)
        # FaInput segments carry start/end in ms and text — feed to align directly.
        segments_in = [
            {
                "start": seg.start_ms / 1000.0,
                "end": seg.end_ms / 1000.0,
                "text": seg.text,
            }
            for seg in item.segments
        ]
        aligned = self._wx.align(
            segments_in, align_model, meta, wave, self._device,
            return_char_alignments=False,
        )
        out_segments: list[Any] = []
        for seg in aligned.get("segments", []):
            words = [
                AsrWord(
                    text=w.get("word") or "",
                    start_ms=int((w.get("start") or 0.0) * 1000),
                    end_ms=int((w.get("end") or 0.0) * 1000),
                    confidence=None,
                )
                for w in seg.get("words", [])
                if w.get("start") is not None
            ]
            if not words:
                continue
            out_segments.append(
                AsrSegment(
                    start_ms=words[0].start_ms,
                    end_ms=words[-1].end_ms,
                    text=(seg.get("text") or "").strip(),
                    speaker=None,
                    words=words,
                )
            )
        return FaOutput(source_id=item.source_id, segments=out_segments)


__all__ = ["WhisperXBackend"]
