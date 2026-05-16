"""Cantonese word/utterance segmentation via ``pycantonese``.

Operates on transcripts that have no spaces (a separate task from ASR);
splits each segment into tokens and distributes the segment's time
span uniformly across the resulting tokens.
"""

from __future__ import annotations

from typing import Any

from batchalign.backends.base import UtSeg, BatchPolicy


class CantoneseWordSegBackend(UtSeg):
    """Word/utterance segmentation for Cantonese transcripts."""

    def __init__(self, *, batch_size: int = 16, batch_window_ms: int = 50) -> None:
        import pycantonese  # type: ignore[import-not-found]

        self._segmenter = pycantonese.segment
        self._policy = BatchPolicy(max_size=batch_size, window_ms=batch_window_ms)

    @property
    def name(self) -> str:
        return "pycantonese:segment"

    @property
    def batch_policy(self) -> BatchPolicy:
        return self._policy

    def call(self, batch: list[Any]) -> list[Any]:
        from batchalign._core.proto import UtSegInput, UtSegOutput, UtteranceSpan, AsrWord

        outputs: list[Any] = []
        for item in batch:
            if not isinstance(item, UtSegInput):
                raise TypeError(
                    f"CantoneseWordSegBackend does not handle: {type(item).__name__}"
                )
            utts: list[Any] = []
            for seg in item.segments:
                tokens = self._segmenter(seg.text)
                # pycantonese has no acoustic alignment; distribute span uniformly.
                n = max(len(tokens), 1)
                span = max(seg.end_ms - seg.start_ms, 0)
                step = span / n
                words = [
                    AsrWord(
                        text=tok,
                        start_ms=int(seg.start_ms + i * step),
                        end_ms=int(seg.start_ms + (i + 1) * step),
                        confidence=None,
                    )
                    for i, tok in enumerate(tokens)
                ]
                utts.append(
                    UtteranceSpan(
                        start_ms=seg.start_ms,
                        end_ms=seg.end_ms,
                        text=" ".join(tokens),
                        words=words,
                    )
                )
            outputs.append(UtSegOutput(source_id=item.source_id, utterances=utts))
        return outputs


__all__ = ["CantoneseWordSegBackend"]
