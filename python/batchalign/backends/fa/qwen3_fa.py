"""Qwen3FaBackend: standalone forced-alignment via Qwen3 ForcedAligner.

Wraps `qwen_asr.Qwen3ForcedAligner` in standalone FA-only mode: the ASR
model is not loaded; the backend treats the existing CHAT main-tier text
as the reference and asks the aligner for word-level timestamps relative
to the audio.

This is the standalone counterpart to the FA already wired in
`backends/asr/qwen3_asr.py` (where the aligner runs as part of ASR).

API surface (verified against the installed `qwen_asr` package):

  Qwen3ForcedAligner.align(audio, text, language) -> List[ForcedAlignResult]

* `audio` accepts `str` (path/url/base64) or `(np.ndarray, sr)` tuples.
  qwen_asr's `normalize_audio_input` downmixes to mono + resamples to
  16 kHz internally, so we hand it `(chunk, native_sr)` and let it do
  the work — no pre-resampling on our side.
* `text` is the reference transcript (joined main-tier word surfaces).
* `language` is an English language name ("English", "Cantonese", …).
  We resolve `FaInput.language` (a `LanguageSpec` filled by
  `FaTaskRunner` from the CHAT's `@Languages:` header) through
  `pycountry` via `LanguageCode.from_str` and map to a Qwen name via
  the shared `qwen_language_name` helper (`backends/_qwen_lang.py`).
  Falls back to English when the spec is unresolved (`PerFile` /
  `Auto`) — the same fallback `FaTaskRunner` implicitly uses when the
  header is absent.

Each `ForcedAlignResult` carries `.items: List[ForcedAlignItem]` whose
`.start_time` / `.end_time` are in seconds.

Per `qwen3_asr.py` MPS guidance, default to CPU and warn on MPS.
"""

from __future__ import annotations

from typing import Any

from batchalign.backends._qwen_lang import qwen_language_name
from batchalign.backends.base import FA, BatchPolicy
from batchalign.lang import LanguageCode


def _resolve_language(item_language: Any) -> str:
    """Map a `FaInput.language` (LanguageSpec) to a Qwen language name.

    `LanguageSpec` is a discriminated union (`LanguageSpecAuto` /
    `LanguageSpecCode` / `LanguageSpecPerFile`). Only `Code` carries a
    resolved ISO-639-3 — CHAT transcripts always use 3-letter codes, so
    the Rust `FaTaskRunner` resolves `@Languages:` to `Code(...)` when
    the header is present. The other two variants mean "no header" or
    "let the backend pick"; both fall back to English here.
    """
    kind = getattr(item_language, "kind", None)
    if kind == "code":
        return qwen_language_name(LanguageCode.from_str(item_language.value))
    return "English"


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
        from qwen_asr import Qwen3ForcedAligner  # type: ignore[import-not-found]

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

        # Standalone FA owns only the alignment model. Constructing an
        # ASR model with this checkpoint and `forced_aligner=model_id`
        # loads the same large checkpoint twice and exposes the aligner only
        # as a nested field. qwen-asr provides the direct typed surface.
        self._aligner = Qwen3ForcedAligner.from_pretrained(
            self._model_id,
            dtype=dtype,
            device_map=device,
        )
        self._policy = BatchPolicy(max_size=batch_size, window_ms=batch_window_ms)

    @property
    def name(self) -> str:
        return f"qwen3-fa:{self._model_id}:{self._device}:v1"

    @property
    def batch_policy(self) -> BatchPolicy:
        return self._policy

    def call(self, batch: list[Any], *, progress: Any = None, **_kwargs: Any) -> list[Any]:
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

        Calls `forced_aligner.align(audio=(chunk, sr), text=reference,
        language=name)` per utterance and reads `results[0].items[i].
        {start_time, end_time}` (seconds) for each reference word.
        """
        if not item.utterances:
            return []
        wave = np.frombuffer(item.audio.pcm_f32le, dtype=np.float32).copy()
        sr = int(item.audio.sample_rate)
        language = _resolve_language(item.language)
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
            hi = min(int(w1 * sr / 1000), wave.shape[0])
            if hi <= lo:
                aligned.append(
                    AsrSegment(
                        start_ms=w0, end_ms=w1, text=utt.text,
                        speaker=getattr(utt, "speaker", None), words=utt.words,
                    )
                )
                continue
            chunk = wave[lo:hi]
            reference = " ".join(w.text for w in words)
            try:
                # qwen_asr resamples + downmixes the `(chunk, sr)` tuple
                # internally — no pre-processing required on our side.
                results = self._aligner.align(
                    audio=(chunk, sr),
                    text=reference,
                    language=language,
                )
            except Exception:
                aligned.append(
                    AsrSegment(
                        start_ms=w0, end_ms=w1, text=utt.text,
                        speaker=getattr(utt, "speaker", None), words=utt.words,
                    )
                )
                continue

            items = list(getattr(results[0], "items", [])) if results else []
            out_words: list[Any] = []
            for w, ali in zip(words, items):
                s = w0 + int(round(float(ali.start_time) * 1000))
                e = w0 + int(round(float(ali.end_time) * 1000))
                # Bound to utterance window (matches Wav2Vec2 behavior).
                s = max(s, w0)
                e = min(e, w1)
                if s >= e:
                    s, e = 0, 0
                out_words.append(
                    AsrWord(text=w.text, start_ms=s, end_ms=e, confidence=None)
                )
            # If qwen returned fewer items than words, the trailing words
            # stay untimed (BA2 parity for `(0, 0)` rendering).
            for w in words[len(items):]:
                out_words.append(
                    AsrWord(text=w.text, start_ms=0, end_ms=0, confidence=None)
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
