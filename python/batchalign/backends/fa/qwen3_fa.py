"""Qwen3FaBackend: standalone forced-alignment via Qwen3 ForcedAligner.

Wraps `qwen_asr.Qwen3ASRModel(forced_aligner="Qwen/Qwen3-ForcedAligner-
0.6B")` in standalone FA-only mode: the ASR side is not used; the
backend treats the existing CHAT main-tier text as the reference and
asks the Qwen3 aligner for word-level timestamps relative to the audio.

This is the standalone counterpart to the FA already wired in
`backends/asr/qwen3_asr.py` (where the aligner runs as part of ASR).

API key resolution: none; the Qwen weights are HuggingFace models, no
service auth required.

Implementation notes:

- The Qwen3 ForcedAligner accepts a text reference + audio; for each
  reference word it emits `(start_ms, end_ms)`. We pass the CHAT main
  tier text as the reference, mirroring how `Wav2Vec2FaBackend` does
  it.
- Per `qwen3_asr.py:79-87`, MPS produces degraded outputs on the 1.7B
  model; default to CPU and warn on MPS.
"""

from __future__ import annotations

from typing import Any

from batchalign.backends.base import FA, BatchPolicy


class Qwen3FaBackend(FA):
    """Standalone Qwen3 forced alignment."""

    def __init__(
        self,
        *,
        model_id: str = "Qwen/Qwen3-ForcedAligner-0.6B",
        device: str = "cpu",
        batch_size: int = 1,
        batch_window_ms: int = 0,
    ) -> None:
        import torch  # type: ignore[import-not-found]
        from qwen_asr import Qwen3ASRModel  # type: ignore[import-not-found]

        self._model_id = model_id
        self._device = device

        if device == "cuda":
            dtype = torch.bfloat16
        elif device == "mps":
            import logging

            logging.getLogger("batchalign.qwen3_fa").warning(
                "Qwen3-FA MPS device requested; empirical testing on the "
                "ASR 1.7B reports degraded output on Apple Silicon. CPU is "
                "the reference path."
            )
            dtype = torch.float16
        else:
            dtype = torch.float32

        # Load ONLY the forced aligner — `forced_aligner=` keyword on
        # Qwen3ASRModel.from_pretrained loads the alignment head; we
        # don't issue any transcribe() calls, just align(). Per the
        # qwen_asr API the same model object hosts both surfaces; we
        # treat ASR transcribe as off and call only the aligner side.
        self._model = Qwen3ASRModel.from_pretrained(
            self._model_id,
            forced_aligner=model_id,
            torch_dtype=dtype,
            device_map=device,
        )
        self._policy = BatchPolicy(max_size=batch_size, window_ms=batch_window_ms)

    @property
    def name(self) -> str:
        return f"qwen3-fa:{self._model_id}:{self._device}:v1"

    @property
    def batch_policy(self) -> BatchPolicy:
        return self._policy

    def call(self, batch: list[Any]) -> list[Any]:
        from batchalign._core.proto import (
            FaInput,
            FaOutput,
            FaWord,
        )

        outputs: list[Any] = []
        for item in batch:
            if not isinstance(item, FaInput):
                raise TypeError(
                    f"Qwen3FaBackend does not handle: {type(item).__name__}"
                )
            # Each FaInput carries the per-segment reference text and the
            # audio chunk. Pass to the aligner.
            spans = self._align_segment(item)
            outputs.append(FaOutput(source_id=item.source_id, words=spans))
        return outputs

    def _align_segment(self, item: Any) -> list[Any]:
        """Run the Qwen3 forced aligner on one FaInput; return FaWord list."""
        from batchalign._core.proto import FaWord

        words = [w for w in getattr(item, "words", []) if w]
        if not words:
            return []
        reference = " ".join(words)
        # The qwen_asr `align` method signature, per
        # `qwen_asr/forced_aligner.py`, returns a list of
        # ForcedAlignItem objects with .text, .start (s), .end (s).
        aligned = self._model.forced_aligner.align(
            audio=item.audio,
            reference=reference,
        )
        out: list[Any] = []
        for ali in aligned:
            out.append(
                FaWord(
                    text=ali.text,
                    start_ms=int(round(ali.start * 1000)),
                    end_ms=int(round(ali.end * 1000)),
                )
            )
        return out


__all__ = ["Qwen3FaBackend"]
