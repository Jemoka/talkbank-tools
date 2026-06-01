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
        import numpy as np  # type: ignore[import-not-found]
        from batchalign._core.proto import (
            AsrSegment,
            AsrWord,
            FaInput,
            FaOutput,
        )

        outputs: list[Any] = []
        for item in batch:
            if not isinstance(item, FaInput):
                raise TypeError(
                    f"Qwen3FaBackend does not handle: {type(item).__name__}"
                )
            aligned_utts = self._align_item(item, np, AsrSegment, AsrWord)
            outputs.append(FaOutput(source_id=item.source_id, utterances=aligned_utts))
        return outputs

    def _align_item(
        self,
        item: Any,
        np: Any,
        AsrSegment: type,
        AsrWord: type,
    ) -> list[Any]:
        """Run Qwen3's forced aligner once per utterance, return refined `AsrSegment`s.

        The qwen_asr `forced_aligner.align(audio=..., reference=...)` API
        returns a list of `ForcedAlignItem`s with `.text`, `.start` (s),
        `.end` (s) per reference word.
        """
        if not item.utterances:
            return []
        wave = np.frombuffer(item.audio.pcm_f32le, dtype=np.float32).copy()
        sr = int(item.audio.sample_rate)
        aligned: list[Any] = []
        for utt in item.utterances:
            words = [w for w in utt.words if w.text]
            w0 = int(getattr(utt, "start_ms", 0) or 0)
            w1 = int(getattr(utt, "end_ms", 0) or 0)
            if not words or w1 <= w0:
                aligned.append(
                    AsrSegment(
                        start_ms=w0, end_ms=w1, text=utt.text,
                        speaker=getattr(utt, "speaker", None), words=utt.words,
                    )
                )
                continue
            lo = int(w0 * sr / 1000)
            hi = int(w1 * sr / 1000)
            chunk = wave[lo:hi]
            reference = " ".join(w.text for w in words)
            try:
                results = self._model.forced_aligner.align(
                    audio=chunk, reference=reference,
                )
            except Exception:
                aligned.append(
                    AsrSegment(
                        start_ms=w0, end_ms=w1, text=utt.text,
                        speaker=getattr(utt, "speaker", None), words=utt.words,
                    )
                )
                continue
            out_words: list[Any] = []
            for w, ali in zip(words, results):
                s = w0 + int(round(ali.start * 1000))
                e = w0 + int(round(ali.end * 1000))
                # Bound to utterance window (matches Wav2Vec2 behavior).
                s = max(s, w0)
                e = min(e, w1)
                if s >= e:
                    s, e = 0, 0
                out_words.append(
                    AsrWord(text=w.text, start_ms=s, end_ms=e, confidence=None)
                )
            timed = [w for w in out_words if w.end_ms > 0]
            aligned.append(
                AsrSegment(
                    start_ms=timed[0].start_ms if timed else w0,
                    end_ms=timed[-1].end_ms if timed else w1,
                    text=utt.text,
                    speaker=getattr(utt, "speaker", None),
                    words=out_words,
                )
            )
        return aligned


__all__ = ["Qwen3FaBackend"]
