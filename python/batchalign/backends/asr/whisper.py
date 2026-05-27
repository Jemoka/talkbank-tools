"""WhisperBackend: local Whisper ASR via HuggingFace ``transformers``.

The default model is ``openai/whisper-large-v3``, matching BA2's
choice (``batchalign2/batchalign/pipelines/asr/whisper.py``). The
dependency on ``transformers``/``torch`` is lazy — constructing a
:class:`WhisperBackend` is free until ``__init__`` actually imports
the libraries.

Whisper provides word-level timestamps via the HF pipeline's
``return_timestamps="word"`` option, which is what we want for the
``AsrSegment.words`` field. Forced alignment is **not** done here —
use :class:`Wav2Vec2FaBackend` from :mod:`batchalign.backends.wav2vec2`
or :class:`WhisperXBackend` for that.
"""

from __future__ import annotations

from typing import Any

from batchalign.backends.base import ASR, BatchPolicy


class WhisperBackend(ASR):
    """Local Whisper ASR backend (single task)."""

    def __init__(
        self,
        model: str = "openai/whisper-large-v3",
        *,
        language: str | None = None,
        batch_size: int = 32,
        batch_window_ms: int = 50,
        device: str | None = None,
        chunk_length_s: int = 30,
    ) -> None:
        from transformers import pipeline  # type: ignore[import-not-found]

        from batchalign.backends.asr._torch_audio import disable_torchcodec

        disable_torchcodec()
        kwargs: dict[str, Any] = {"chunk_length_s": chunk_length_s}
        if device is not None:
            kwargs["device"] = device
        self._pipe = pipeline(
            "automatic-speech-recognition",
            model=model,
            **kwargs,
        )
        self._model = model
        # Fallback language when the per-call input ships `Auto` (the runner
        # always does); lets the CLI pin `--language`. `None`/"auto" means
        # let Whisper auto-detect.
        self._language = None if language in (None, "auto") else language
        self._policy = BatchPolicy(max_size=batch_size, window_ms=batch_window_ms)

    @property
    def name(self) -> str:
        return f"whisper:{self._model}"

    @property
    def batch_policy(self) -> BatchPolicy:
        return self._policy

    def call(self, batch: list[Any]) -> list[Any]:
        from batchalign._core.proto import AsrInput, AsrOutput

        outputs: list[Any] = []
        for item in batch:
            if not isinstance(item, AsrInput):
                raise TypeError(
                    f"WhisperBackend does not handle input type: {type(item).__name__}"
                )
            outputs.append(self._transcribe(item, AsrOutput))
        return outputs

    # ----- internals -----------------------------------------------------

    def _transcribe(self, item: Any, AsrOutput: type) -> Any:
        import numpy as np  # type: ignore[import-not-found]
        from batchalign._core.proto import AsrSegment, AsrWord

        wave = np.frombuffer(item.audio.pcm_f32le, dtype=np.float32)
        language = (
            item.language.value if item.language.kind == "code" else self._language
        )
        gen_kwargs: dict[str, Any] = {}
        if language is not None:
            gen_kwargs["language"] = language

        result = self._pipe(
            {"array": wave, "sampling_rate": item.audio.sample_rate},
            return_timestamps="word",
            generate_kwargs=gen_kwargs or None,
        )

        # HF returns
        #   {"text": "...", "chunks": [{"timestamp": (s, e), "text": "..."}, ...]}
        # We package every word as its own segment. The Rust UtSeg runner
        # later merges these into utterances; emitting word-level is the
        # most faithful shape we can produce from a single Whisper pass.
        words: list[Any] = []
        for chunk in result.get("chunks", []):
            ts = chunk.get("timestamp") or (0.0, 0.0)
            start_s = ts[0] or 0.0
            end_s = ts[1] or start_s
            words.append(
                AsrWord(
                    text=(chunk.get("text") or "").strip(),
                    start_ms=int(start_s * 1000),
                    end_ms=int(end_s * 1000),
                    confidence=None,
                )
            )

        if not words:
            return AsrOutput(source_id=item.source_id, segments=[])

        full_text = (result.get("text") or " ".join(w.text for w in words)).strip()
        segment = AsrSegment(
            start_ms=words[0].start_ms,
            end_ms=words[-1].end_ms,
            text=full_text,
            speaker=None,
            words=words,
        )
        return AsrOutput(source_id=item.source_id, segments=[segment])


__all__ = ["WhisperBackend"]
