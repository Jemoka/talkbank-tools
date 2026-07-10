"""Malayalam ASR via the Hugging Face Wav2Vec2 CTC pipeline.

The default checkpoint is ``gvs/wav2vec2-large-xlsr-malayalam``.  Unlike
sequence-to-sequence ASR models, a CTC pipeline can derive word timestamps
directly from its frame-level token offsets with ``return_timestamps="word"``.
"""

from __future__ import annotations

from typing import Any

from batchalign.backends.base import ASR, BatchPolicy


DEFAULT_MODEL = "gvs/wav2vec2-large-xlsr-malayalam"


class MalayalamWav2Vec2Backend(ASR):
    """Local Malayalam Wav2Vec2 ASR with CTC word timestamps."""

    def __init__(
        self,
        model: str = DEFAULT_MODEL,
        *,
        device: str | None = None,
        chunk_length_s: int = 30,
        stride_length_s: int | tuple[int, int] = (4, 2),
        batch_size: int = 1,
        batch_window_ms: int = 0,
    ) -> None:
        from transformers import pipeline

        from batchalign.backends.asr._torch_audio import disable_torchcodec

        disable_torchcodec()
        kwargs: dict[str, Any] = {
            "chunk_length_s": chunk_length_s,
            "stride_length_s": stride_length_s,
        }
        if device is not None:
            kwargs["device"] = device
        self._pipe = pipeline(
            "automatic-speech-recognition",
            model=model,
            **kwargs,
        )
        self._model = model
        self._policy = BatchPolicy(max_size=batch_size, window_ms=batch_window_ms)

    @property
    def name(self) -> str:
        return f"malayalam-wav2vec2:{self._model}:v6"

    @property
    def batch_policy(self) -> BatchPolicy:
        return self._policy

    def call(self, batch: list[Any], *, progress: Any = None, **_kwargs: Any) -> list[Any]:
        from batchalign._core.proto import AsrInput, AsrOutput

        outputs: list[Any] = []
        for item in batch:
            if not isinstance(item, AsrInput):
                raise TypeError(
                    "MalayalamWav2Vec2Backend does not handle input type: "
                    f"{type(item).__name__}"
                )
            outputs.append(self._transcribe(item, AsrOutput))
        return outputs

    def _transcribe(self, item: Any, AsrOutput: type) -> Any:
        import numpy as np

        from batchalign._core.proto import AsrSegment, AsrWord
        from batchalign.backends.asr._torch_audio import ctc_timestamp_scale

        # Transformers converts this array to a torch tensor; make it writable
        # so PyTorch does not warn about undefined behavior on a bytes-backed
        # read-only NumPy view.
        wave = np.frombuffer(item.audio.pcm_f32le, dtype=np.float32).copy()
        result = self._pipe(
            {"array": wave, "sampling_rate": item.audio.sample_rate},
            return_timestamps="word",
        )

        chunks = result.get("chunks", [])
        timestamp_scale = ctc_timestamp_scale(
            chunks,
            duration_s=wave.size / item.audio.sample_rate,
        )

        words: list[Any] = []
        for chunk in chunks:
            text = str(chunk.get("text") or "").strip()
            timestamp = chunk.get("timestamp")
            if not text or not timestamp or timestamp[0] is None:
                continue
            start_s = float(timestamp[0]) * timestamp_scale
            end_s = (
                float(timestamp[1]) * timestamp_scale
                if timestamp[1] is not None
                else start_s
            )
            words.append(
                AsrWord(
                    text=text,
                    start_ms=round(start_s * 1000),
                    end_ms=round(end_s * 1000),
                    confidence=None,
                )
            )

        if not words:
            return AsrOutput(source_id=item.source_id, segments=[])

        text = str(result.get("text") or " ".join(word.text for word in words)).strip()
        segment = AsrSegment(
            start_ms=words[0].start_ms,
            end_ms=words[-1].end_ms,
            text=text,
            speaker=None,
            words=words,
        )
        return AsrOutput(source_id=item.source_id, segments=[segment])

__all__ = ["DEFAULT_MODEL", "MalayalamWav2Vec2Backend"]
