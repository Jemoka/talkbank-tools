"""Qwen3AsrBackend: Qwen3-ASR (Alibaba open-weight ASR LLM).

Faithful port of tbtbt's `_qwen_asr` / `_qwen_common`
(`batchalign/inference/languages/cantonese/_qwen_asr.py`). Wraps the
`qwen-asr` PyPI package's `Qwen3ASRModel.from_pretrained`, runs
inference with an explicit language label, and projects the model's
per-utterance text + optional word timestamps into `AsrSegment` /
`AsrWord` records.

Engine selection rationale: Qwen3-ASR is an open-weight Cantonese-capable
ASR model from Alibaba; external evaluations on per-utterance Cantonese
child speech report competitive CER versus the major cloud APIs and
versus Cantonese-finetuned Whisper variants.

Device defaults to `cpu`. Apple Silicon hosts have no CUDA; empirical
testing (tbtbt 2026-05-26) found MPS-backed inference degraded on the
1.7B model (transformers MPS attention path incomplete for Qwen3).
"""

from __future__ import annotations

from typing import Any

from batchalign.backends.base import ASR, BatchPolicy
from batchalign.backends._qwen_lang import qwen_language_name
from batchalign.lang import LanguageCode


class Qwen3AsrBackend(ASR):
    """Qwen3-ASR (tbtbt parity)."""

    def __init__(
        self,
        *,
        language: LanguageCode,
        model_id: str = "Qwen/Qwen3-ASR-1.7B",
        # Word-level timing requires a separately-loaded Qwen3 forced
        # aligner. The canonical companion checkpoint is
        # `Qwen/Qwen3-ForcedAligner-0.6B` (see
        # `qwen_asr/cli/demo.py:131`). Pass `forced_aligner=None` for
        # text-only output (no `%wor:` tier feasible without it).
        forced_aligner: str | None = "Qwen/Qwen3-ForcedAligner-0.6B",
        device: str = "cpu",
        max_inference_batch_size: int = 32,
        max_new_tokens: int = 4096,
        batch_size: int = 1,
        batch_window_ms: int = 0,
    ) -> None:
        import torch  # type: ignore[import-not-found]
        from qwen_asr import Qwen3ASRModel  # type: ignore[import-not-found]

        self._lang = language.alpha_3
        self._model_id = model_id
        self._device = device

        self._qwen_language = qwen_language_name(language)

        if device == "cuda":
            dtype = torch.bfloat16
        elif device == "mps":
            # User opted into MPS; warn but honor. tbtbt 2026-05-26 reports
            # degraded output (~78% CER vs ~57% on CPU) for the 1.7B model.
            import logging

            logging.getLogger("batchalign.qwen3").warning(
                "Qwen3-ASR MPS device requested; empirical testing reports "
                "degraded output on 1.7B. Use CPU for reference quality."
            )
            dtype = torch.float16
        else:
            dtype = torch.float32

        self._forced_aligner_id = forced_aligner
        self._model = Qwen3ASRModel.from_pretrained(
            self._model_id,
            forced_aligner=forced_aligner,
            torch_dtype=dtype,
            device_map=device,
            max_inference_batch_size=max_inference_batch_size,
            # `max_new_tokens` caps per-chunk generation. The qwen-asr
            # package handles internal long-audio chunking, so each chunk's
            # text is bounded by this. 4096 is generous for ~5-60s chunks.
            max_new_tokens=max_new_tokens,
        )
        self._policy = BatchPolicy(max_size=batch_size, window_ms=batch_window_ms)

    @property
    def name(self) -> str:
        return f"qwen3-asr:{self._model_id}:{self._lang}:v1"

    @property
    def batch_policy(self) -> BatchPolicy:
        return self._policy

    def call(self, batch: list[Any], *, progress: Any = None, **_kwargs: Any) -> list[Any]:
        from batchalign._core.proto import AsrInput, AsrOutput

        outputs: list[Any] = []
        for item in batch:
            if not isinstance(item, AsrInput):
                raise TypeError(
                    f"Qwen3AsrBackend does not handle: {type(item).__name__}"
                )
            outputs.append(self._transcribe(item, AsrOutput))
        return outputs

    def _transcribe(self, item: Any, AsrOutput: type) -> Any:
        import pathlib
        import tempfile

        import numpy as np  # type: ignore[import-not-found]
        import soundfile as sf  # type: ignore[import-not-found]

        from batchalign._core.proto import AsrSegment, AsrWord

        # Prefer the original media file (byte-identical to tbtbt's path);
        # fall back to dumping the decoded PCM to a temp WAV.
        orig = str(item.source_id) if item.source_id is not None else ""
        local_path = orig
        tmp_path = ""
        if not (orig and pathlib.Path(orig).is_file()):
            with tempfile.NamedTemporaryFile(suffix=".wav", delete=False) as tmp:
                tmp_path = tmp.name
            wave = np.frombuffer(item.audio.pcm_f32le, dtype=np.float32).copy()
            sf.write(tmp_path, wave, int(item.audio.sample_rate))
            local_path = tmp_path

        try:
            # `return_time_stamps=True` requires `forced_aligner` at model
            # init (we pass `Qwen/Qwen3-ForcedAligner-0.6B` by default).
            # When the operator opts out of the FA model
            # (`forced_aligner=None`), we still call transcribe but skip
            # timestamps — word-level timing can come from a chained FA
            # backend (`whisper_fa`, `wav2vec`) in a downstream task.
            want_timestamps = self._forced_aligner_id is not None
            results = self._model.transcribe(
                audio=local_path,
                # tbtbt parity: pass the explicit label rather than relying on
                # auto-detect — auto on a known-language input risks the model
                # misclassifying short / low-energy segments.
                language=self._qwen_language,
                return_time_stamps=want_timestamps,
            )
        finally:
            if tmp_path:
                try:
                    pathlib.Path(tmp_path).unlink()
                except OSError:
                    pass

        # qwen-asr may return either a single result or a list. Normalize.
        if not isinstance(results, list):
            results = [results]

        words: list[Any] = []
        # Each "result" is a Qwen3-ASR utterance with `.text` and optionally
        # `.time_stamps.items` (list of {.start_time,.end_time,.text}) when
        # the FA aligner is loaded. Times are seconds (float) — `_qwen_common`
        # / `qwen3_forced_aligner.ForcedAlignItem` shape.
        for raw in results:
            text = str(getattr(raw, "text", "") or "")
            ts_container = getattr(raw, "time_stamps", None)
            ts_items = getattr(ts_container, "items", None) if ts_container else None
            if ts_items:
                for ts in ts_items:
                    start_s = float(getattr(ts, "start_time", 0.0))
                    end_s = float(getattr(ts, "end_time", 0.0))
                    word = str(getattr(ts, "text", "") or "")
                    if not word.strip():
                        continue
                    words.append(
                        AsrWord(
                            text=word,
                            start_ms=int(start_s * 1000),
                            end_ms=int(end_s * 1000),
                            confidence=None,
                        )
                    )
            elif text.strip():
                # No per-word timestamps — emit the whole-segment text as one
                # word with no timing. tbtbt parity (CJK char-level
                # tokenization is left to BA3's standard Cantonese path).
                words.append(
                    AsrWord(
                        text=text,
                        start_ms=0,
                        end_ms=0,
                        confidence=None,
                    )
                )

        if not words:
            return AsrOutput(source_id=item.source_id, segments=[])

        timed = [w for w in words if w.end_ms > 0]
        # tbtbt joins yue with no space (char-level downstream); other
        # languages use a space. The text shape matches the AsrSegment
        # contract the UtSeg pairing reads (`" ".join(words)` upstream
        # — see build_chat_from_asr).
        joiner = "" if self._lang == "yue" else " "
        seg = AsrSegment(
            start_ms=timed[0].start_ms if timed else 0,
            end_ms=timed[-1].end_ms if timed else 0,
            text=joiner.join(w.text for w in words).strip(),
            speaker=None,
            words=words,
        )
        return AsrOutput(source_id=item.source_id, segments=[seg])


__all__ = ["Qwen3AsrBackend"]
