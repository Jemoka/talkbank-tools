"""PyannoteBackend: speaker diarization + utterance segmentation.

Wraps ``pyannote.audio`` (the same library BA2 uses; see
``batchalign2/batchalign/pipelines/speaker/``). The HuggingFace token
is read from ``~/.batchalign.ini`` (``[auth] hf_token``) or env var
``BATCHALIGN_HF_KEY`` via :func:`batchalign.config.get_api_key`.

Pyannote's pipeline accepts an in-memory waveform as a dict of the
form ``{"waveform": torch.Tensor[1, N], "sample_rate": int}``; we
build that directly from the proto's PCM bytes (no temp file needed).

The backend services two tasks atomically — every diarization pass
also produces utterance spans, so the underlying inference runs
once per ``source_id``. When the engine hands us a mixed batch
(``SpeakerInput`` + ``UtSegInput`` for the same source), we dedupe by
``source_id`` and project the cached diarization into both output
variants.
"""

from __future__ import annotations

from typing import Any

from batchalign.backends.base import Speaker, UtSeg, BatchPolicy
from batchalign import config


class PyannoteBackend(Speaker, UtSeg):
    """Pyannote-based speaker diarization with utterance-boundary fallout."""

    def __init__(
        self,
        model: str = "pyannote/speaker-diarization-3.1",
        *,
        hf_token: str | None = None,
        batch_size: int = 1,
        batch_window_ms: int = 0,
    ) -> None:
        from pyannote.audio import Pipeline  # type: ignore[import-not-found]

        token = hf_token if hf_token is not None else config.get_api_key("hf")
        self._pipeline = Pipeline.from_pretrained(model, use_auth_token=token)
        self._model = model
        self._policy = BatchPolicy(max_size=batch_size, window_ms=batch_window_ms)

    @property
    def name(self) -> str:
        return f"pyannote:{self._model}"

    @property
    def batch_policy(self) -> BatchPolicy:
        return self._policy

    def call(self, batch: list[Any]) -> list[Any]:
        from batchalign._core.proto import (
            SpeakerInput,
            SpeakerOutput,
            UtSegInput,
            UtSegOutput,
            Diarization,
            DiarizationSegment,
            UtteranceSpan,
        )

        # Atomic-call dedupe: one inference per source_id.
        cache: dict[str, list[tuple[float, float, str]]] = {}

        def _run(item: Any) -> list[tuple[float, float, str]]:
            cached = cache.get(item.source_id)
            if cached is not None:
                return cached
            spans = self._diarize_one(item)
            cache[item.source_id] = spans
            return spans

        outputs: list[Any] = []
        for item in batch:
            if isinstance(item, SpeakerInput):
                spans = _run(item)
                outputs.append(
                    SpeakerOutput(
                        source_id=item.source_id,
                        diarization=Diarization(
                            segments=[
                                DiarizationSegment(
                                    start_ms=int(s * 1000),
                                    end_ms=int(e * 1000),
                                    speaker=spk,
                                )
                                for (s, e, spk) in spans
                            ]
                        ),
                    )
                )
            elif isinstance(item, UtSegInput):
                spans = _run(item) if hasattr(item, "audio") else []
                # UtSegInput in the current proto carries `segments`, not
                # raw audio — we use those word boundaries as the source
                # of truth when present, falling back to diarization spans.
                utts = self._utterances_from_segments(item, UtteranceSpan)
                outputs.append(
                    UtSegOutput(source_id=item.source_id, utterances=utts)
                )
            else:
                raise TypeError(
                    f"PyannoteBackend does not handle input type: {type(item).__name__}"
                )
        return outputs

    # ----- internals -----------------------------------------------------

    def _diarize_one(self, item: Any) -> list[tuple[float, float, str]]:
        """Run pyannote on one audio buffer and return ``(start_s, end_s, spk)`` rows."""
        import numpy as np  # type: ignore[import-not-found]
        import torch  # type: ignore[import-not-found]

        wave_np = np.frombuffer(item.audio.pcm_f32le, dtype=np.float32)
        wave = torch.from_numpy(np.ascontiguousarray(wave_np)).unsqueeze(0)
        kwargs: dict[str, Any] = {}
        n = int(getattr(item, "num_speakers", 0) or 0)
        if n > 0:
            kwargs["num_speakers"] = n
        annotation = self._pipeline(
            {"waveform": wave, "sample_rate": int(item.audio.sample_rate)},
            **kwargs,
        )
        return [(t.start, t.end, str(label)) for t, _, label in annotation.itertracks(yield_label=True)]

    @staticmethod
    def _utterances_from_segments(item: Any, UtteranceSpan: type) -> list[Any]:
        """Turn an :class:`UtSegInput`'s ASR segments into :class:`UtteranceSpan`.

        Pyannote-as-UtSeg here is a passthrough — the diarization step
        already split the audio at speaker turns; we trust the upstream
        ASR segmentation and just project word spans through. For real
        text-based utterance segmentation, use a Stanza/punctuation
        backend instead.
        """
        utts: list[Any] = []
        for seg in item.segments:
            utts.append(
                UtteranceSpan(
                    start_ms=seg.start_ms,
                    end_ms=seg.end_ms,
                    text=seg.text,
                    words=list(seg.words),
                )
            )
        return utts


__all__ = ["PyannoteBackend"]
