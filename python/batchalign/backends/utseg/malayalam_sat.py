"""Malayalam utterance segmentation using wtpsplit's SaT model."""

from __future__ import annotations

from typing import Any

from batchalign.backends.base import BatchPolicy, UtSeg


class MalayalamSaTBackend(UtSeg):
    """Split Malayalam ASR text with ``wtpsplit.SaT``."""

    def __init__(
        self,
        model: str = "sat-3l-sm",
        *,
        batch_size: int = 8,
        batch_window_ms: int = 50,
    ) -> None:
        from wtpsplit import SaT

        self._model_id = model
        self._segmenter = SaT(model)
        self._policy = BatchPolicy(max_size=batch_size, window_ms=batch_window_ms)

    @property
    def name(self) -> str:
        return f"malayalam-sat:{self._model_id}:wordts1"

    @property
    def batch_policy(self) -> BatchPolicy:
        return self._policy

    def call(self, batch: list[Any], *, progress: Any = None, **_kwargs: Any) -> list[Any]:
        from batchalign._core.proto import UtSegInput, UtSegOutput, UtteranceSpan

        outputs: list[Any] = []
        for item in batch:
            if not isinstance(item, UtSegInput):
                raise TypeError(
                    f"MalayalamSaTBackend does not handle: {type(item).__name__}"
                )

            spans: list[Any] = []
            for segment in item.segments:
                sentences = [
                    sentence.strip()
                    for sentence in self._segmenter.split((segment.text or "").strip())
                    if sentence.strip()
                ]
                cursor = 0
                source_words = list(segment.words or [])
                for index, sentence in enumerate(sentences):
                    count = len(sentence.split())
                    if index == len(sentences) - 1:
                        words = source_words[cursor:]
                    else:
                        words = source_words[cursor : cursor + count]
                    cursor += len(words)
                    timed = [word for word in words if word.end_ms > word.start_ms]
                    spans.append(
                        UtteranceSpan(
                            start_ms=timed[0].start_ms if timed else 0,
                            end_ms=timed[-1].end_ms if timed else 0,
                            text=sentence,
                            words=words,
                        )
                    )
            outputs.append(UtSegOutput(source_id=item.source_id, utterances=spans))
        return outputs


__all__ = ["MalayalamSaTBackend"]
