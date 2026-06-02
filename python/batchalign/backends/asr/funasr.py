"""Open-source FunASR ASR backend (Paraformer-class Mandarin/Cantonese).

https://github.com/modelscope/FunASR. Models are downloaded from
ModelScope on first use; fully offline thereafter.
"""

from __future__ import annotations

from typing import Any

from batchalign.backends.base import ASR, BatchPolicy


class FunAsrBackend(ASR):
    """FunASR with VAD + ASR + punctuation, local."""

    def __init__(
        self,
        model: str = "paraformer-zh",
        *,
        vad_model: str = "fsmn-vad",
        punc_model: str = "ct-punc",
        device: str | None = None,
        batch_size: int = 1,
        batch_window_ms: int = 0,
    ) -> None:
        from funasr import AutoModel  # type: ignore[import-not-found]

        kwargs: dict[str, Any] = {"model": model, "vad_model": vad_model, "punc_model": punc_model}
        if device is not None:
            kwargs["device"] = device
        self._model = AutoModel(**kwargs)
        self._model_id = model
        self._policy = BatchPolicy(max_size=batch_size, window_ms=batch_window_ms)

    @property
    def name(self) -> str:
        return f"funasr:{self._model_id}"

    @property
    def batch_policy(self) -> BatchPolicy:
        return self._policy

    def call(self, batch: list[Any], *, progress: Any = None, **_kwargs: Any) -> list[Any]:
        import numpy as np  # type: ignore[import-not-found]
        from batchalign._core.proto import AsrInput, AsrOutput, AsrSegment, AsrWord

        outputs: list[Any] = []
        for item in batch:
            if not isinstance(item, AsrInput):
                raise TypeError(
                    f"FunAsrBackend does not handle: {type(item).__name__}"
                )
            wave_np = np.frombuffer(item.audio.pcm_f32le, dtype=np.float32)
            results = self._model.generate(input=wave_np, sampling_rate=int(item.audio.sample_rate))
            words: list[Any] = []
            text_parts: list[str] = []
            for res in results:
                text_parts.append(res.get("text", ""))
                for char, (s, e) in zip(res.get("text", ""), res.get("timestamp") or []):
                    words.append(
                        AsrWord(text=char, start_ms=int(s), end_ms=int(e), confidence=None)
                    )
            if not words:
                outputs.append(AsrOutput(source_id=item.source_id, segments=[]))
                continue
            seg = AsrSegment(
                start_ms=words[0].start_ms,
                end_ms=words[-1].end_ms,
                text="".join(text_parts),
                speaker=None,
                words=words,
            )
            outputs.append(AsrOutput(source_id=item.source_id, segments=[seg]))
        return outputs


__all__ = ["FunAsrBackend"]
