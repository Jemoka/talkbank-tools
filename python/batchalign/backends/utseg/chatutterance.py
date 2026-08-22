"""CHATUtteranceBackend: BA2's BERT utterance segmenter as a `UtSeg` backend.

BA2 applies the TalkBank `CHATUtterance-*` BERT model to EVERY ASR engine's
output (`process_generation` → `retokenize_with_engine`) to carve a speaker
turn's word blob into utterances. BA3 models that as a separate `UtSeg` task,
so this backend is the BA2 segmentation pairing: the transcribe recipe runs
`[Asr, (Speaker,) UtSeg=CHATUtterance]`, and the UtSeg runner hands each
utterance's text here to be split.

It's a `UtSeg` backend (no new backend *type*); it reuses the
`BertUtteranceModel` defined alongside the CHATWhisper ASR backend. The model
(`talkbank/CHATUtterance-en`, a ~440 MB BERT) is small enough to run where the
6 GB CHATWhisper ASR model will not, which is why `rev` ASR + this segmenter
is the disk-light path to transcribe parity.
"""

from __future__ import annotations

from typing import Any

from batchalign.backends.base import UtSeg, BatchPolicy
from batchalign.backends.utseg.cleanup import (
    SUPPORT_SUFFIX,
    clean_utterance,
    load_cleanup,
)

# BA2 model resolution for the utterance segmenter (models/resolve.py).
_UTTERANCE_RESOLVE = {
    "eng": "talkbank/CHATUtterance-en",
    "en": "talkbank/CHATUtterance-en",
    "zho": "talkbank/CHATUtterance-zh_CN",
    "zh": "talkbank/CHATUtterance-zh_CN",
    "zh-hans": "talkbank/CHATUtterance-zh_CN",
    "yue": "PolyU-AngelChanLab/Cantonese-Utterance-Segmentation",
}


class CHATUtteranceBackend(UtSeg):
    """BERT utterance segmentation (BA2 `CHATUtterance`)."""

    def __init__(
        self,
        *,
        lang: str = "eng",
        batch_size: int = 32,
        batch_window_ms: int = 50,
        cantonese_inference: bool = False,
        segmenter: Any | None = None,
    ) -> None:
        from batchalign.backends.asr.chatwhisper import (
            BertCantoneseUtteranceModel,
            BertUtteranceModel,
        )

        model = _UTTERANCE_RESOLVE.get(lang)
        if model is None:
            raise ValueError(
                f"no CHATUtterance segmentation model for language {lang!r}; "
                f"known: {sorted(_UTTERANCE_RESOLVE)}"
            )
        self._lang = lang
        self._model_id = model
        # Cantonese uses a distinct inference (particle-chunking, char-level).
        # `cantonese_inference` forces it even for a non-yue model: BA2's
        # FunAudioEngine always segments with BertCantoneseUtteranceModel
        # (even paraformer-zh), so the funaudio path opts in to match.
        self._cantonese = lang == "yue" or cantonese_inference
        if segmenter is not None:
            self._segmenter = segmenter
        elif self._cantonese:
            self._segmenter = BertCantoneseUtteranceModel(model)
        else:
            self._segmenter = BertUtteranceModel(model)
        # Disfluency / replacement table for this language (BA2 pairs the
        # disfluency stage with utterance segmentation in the ASR pipeline).
        self._cleanup = load_cleanup(SUPPORT_SUFFIX.get(lang, lang))
        self._policy = BatchPolicy(max_size=batch_size, window_ms=batch_window_ms)

    @property
    def name(self) -> str:
        # Bump the trailing tag when segmentation/cleanup behaviour changes so
        # the result cache invalidates. Tags so far:
        #   * `disfl2` — + retrace [/] marking
        #   * `tsdist` — + utterance-level timestamp distribution from the
        #     parent AsrSegment's [start_ms, end_ms] across the split spans
        #   * `nodot1` — strip internal periods left behind by Punkt
        #     abbreviation handling
        #   * `wordts1` — carry fixed ASR word timings through split spans
        #   * `typed2` — typed word assignments; preserve CHAT case/structure
        canto = ":canto" if self._cantonese and self._lang != "yue" else ""
        return f"chatutterance:{self._model_id}:typed2{canto}"

    @property
    def batch_policy(self) -> BatchPolicy:
        return self._policy

    def call(self, batch: list[Any], *, progress: Any = None, **_kwargs: Any) -> list[Any]:
        from batchalign._core.proto import AsrWord, UtSegInput, UtSegOutput, UtteranceSpan

        for item in batch:
            if not isinstance(item, UtSegInput):
                raise TypeError(
                    f"CHATUtteranceBackend does not handle: {type(item).__name__}"
                )

        batched_assignments: dict[tuple[int, int], list[int]] = {}
        batch_predictor = getattr(
            self._segmenter, "predict_assignments_batch", None
        )
        if not getattr(self, "_cantonese", False) and batch_predictor is not None:
            locations: list[tuple[int, int]] = []
            word_sequences: list[list[str]] = []
            for item_index, item in enumerate(batch):
                for segment_index, seg in enumerate(item.segments):
                    if not (seg.text or "").strip():
                        continue
                    locations.append((item_index, segment_index))
                    word_sequences.append(
                        [str(word.text) for word in seg.words]
                    )
            assignment_batches = batch_predictor(word_sequences)
            batched_assignments.update(zip(locations, assignment_batches))

        outputs: list[Any] = []
        for item_index, item in enumerate(batch):
            spans: list[Any] = []
            for segment_index, seg in enumerate(item.segments):
                text = (seg.text or "").strip()
                if not text:
                    continue
                # The gold pipeline consumes word-level group assignments,
                # then applies them to the existing typed CHAT utterance. This
                # preserves capitalization, retraces, dependent tiers, and the
                # parent's bullet. Keep the older sentence reconstruction only
                # for Cantonese's distinct model and test doubles that expose
                # the legacy callable interface.
                predictor = getattr(self._segmenter, "predict_assignments", None)
                if not getattr(self, "_cantonese", False) and predictor is not None:
                    source_words = list(seg.words)
                    assignments = batched_assignments.get(
                        (item_index, segment_index)
                    )
                    if assignments is None:
                        assignments = predictor(
                            [str(word.text) for word in source_words]
                        )
                    if len(assignments) == len(source_words):
                        spans.extend(
                            _spans_from_assignments(
                                source_words,
                                assignments,
                                UtteranceSpan,
                            )
                        )
                        continue
                # Step 1: collect cleaned sentences (BERT segmenter + BA2
                # disfluency [`uh → &-uh`] + retrace [`[/]`] marking).
                sentences: list[str] = []
                for sentence in self._segmenter(text):
                    sentence = sentence.strip()
                    if not sentence:
                        continue
                    if sentence[-1:] in ".?!":
                        sentence = sentence[:-1].replace(".", "") + sentence[-1]
                    else:
                        sentence = sentence.replace(".", "")
                    sentences.append(
                        clean_utterance(sentence, self._cleanup, self._lang)
                    )
                if not sentences:
                    continue
                # Step 2: distribute the PARENT utterance's [start_ms, end_ms]
                # proportionally across the segmented sentences by character
                # count. This gives every split sub-utterance a usable bullet,
                # so transcripts produced from segments with timing (FunAudio,
                # Tencent, Qwen3-ASR + FA, …) carry utterance-level
                # timestamps end-to-end. When source words carry fixed ASR
                # timings, those words are sliced into the emitted spans below.
                #
                # When the parent has no timing (`end_ms == 0`), emit
                # zero-timed spans (no bullet downstream) so we don't
                # fabricate bullets out of nothing.
                bounds = _distributed_bounds(seg.start_ms, seg.end_ms, sentences)
                cursor = 0
                for sent, (fallback_start, fallback_end) in zip(sentences, bounds):
                    tokens = _timing_tokens(sent)
                    fixed_words = []
                    for token, src in zip(tokens, seg.words[cursor:cursor + len(tokens)]):
                        fixed_words.append(
                            AsrWord(
                                text=token,
                                start_ms=src.start_ms,
                                end_ms=src.end_ms,
                                confidence=src.confidence,
                            )
                        )
                    cursor += len(tokens)
                    timed = [w for w in fixed_words if w.end_ms > w.start_ms]
                    if timed:
                        start_ms, end_ms = timed[0].start_ms, timed[-1].end_ms
                    else:
                        start_ms, end_ms = fallback_start, fallback_end
                    spans.append(
                        UtteranceSpan(
                            start_ms=start_ms,
                            end_ms=end_ms,
                            text=sent,
                            words=fixed_words,
                        )
                    )
            outputs.append(UtSegOutput(source_id=item.source_id, utterances=spans))
        return outputs


def _spans_from_assignments(
    words: list[Any], assignments: list[int], UtteranceSpan: type
) -> list[Any]:
    """Partition original words without rewriting their text or timing."""
    if not words:
        return []

    spans: list[Any] = []
    group_start = 0
    for index in range(1, len(words) + 1):
        if index < len(words) and assignments[index] == assignments[group_start]:
            continue
        grouped_words = words[group_start:index]
        timed = [word for word in grouped_words if word.end_ms > word.start_ms]
        spans.append(
            UtteranceSpan(
                start_ms=timed[0].start_ms if timed else 0,
                end_ms=timed[-1].end_ms if timed else 0,
                text=" ".join(str(word.text) for word in grouped_words),
                words=grouped_words,
            )
        )
        group_start = index
    return spans


def _distributed_bounds(
    start_ms: int,
    end_ms: int,
    sentences: list[str],
) -> list[tuple[int, int]]:
    if end_ms <= start_ms:
        return [(0, 0) for _ in sentences]
    parent_dur = end_ms - start_ms
    total_chars = sum(len(s) for s in sentences) or 1
    cur = start_ms
    bounds: list[tuple[int, int]] = []
    for i, sent in enumerate(sentences):
        if i == len(sentences) - 1:
            end = end_ms
        else:
            end = cur + round(len(sent) / total_chars * parent_dur)
        bounds.append((cur, end))
        cur = end
    return bounds


def _timing_tokens(text: str) -> list[str]:
    """Return alignable output tokens, excluding CHAT markup and terminators."""
    tokens: list[str] = []
    for raw in text.replace("<", " < ").replace(">", " > ").split():
        tok = raw.strip()
        if not tok or tok in {"<", ">", "[/]", "[//]", ".", "?", "!", ","}:
            continue
        if tok.startswith("[") and tok.endswith("]"):
            continue
        tok = tok.strip("<>").rstrip(".?!,;:")
        if tok:
            tokens.append(tok)
    return tokens


__all__ = ["CHATUtteranceBackend"]
