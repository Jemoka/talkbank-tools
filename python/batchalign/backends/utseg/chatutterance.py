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
        batch_size: int = 8,
        batch_window_ms: int = 50,
        cantonese_inference: bool = False,
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
        if self._cantonese:
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
        canto = ":canto" if self._cantonese and self._lang != "yue" else ""
        return f"chatutterance:{self._model_id}:disfl2:tsdist{canto}"

    @property
    def batch_policy(self) -> BatchPolicy:
        return self._policy

    def call(self, batch: list[Any], *, progress: Any = None, **_kwargs: Any) -> list[Any]:
        from batchalign._core.proto import UtSegInput, UtSegOutput, UtteranceSpan

        outputs: list[Any] = []
        for item in batch:
            if not isinstance(item, UtSegInput):
                raise TypeError(
                    f"CHATUtteranceBackend does not handle: {type(item).__name__}"
                )
            spans: list[Any] = []
            for seg in item.segments:
                text = (seg.text or "").strip()
                if not text:
                    continue
                # Step 1: collect cleaned sentences (BERT segmenter + BA2
                # disfluency [`uh → &-uh`] + retrace [`[/]`] marking).
                sentences: list[str] = []
                for sentence in self._segmenter(text):
                    sentence = sentence.strip()
                    if not sentence:
                        continue
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
                # timestamps end-to-end. Word-level timings would require
                # mapping per-word bullets through the segmenter; that's a
                # follow-up — utterance-level is the user-stated bar.
                #
                # When the parent has no timing (`end_ms == 0`), emit
                # zero-timed spans (no bullet downstream) so we don't
                # fabricate bullets out of nothing.
                if seg.end_ms > seg.start_ms:
                    parent_start = seg.start_ms
                    parent_dur = seg.end_ms - seg.start_ms
                    total_chars = sum(len(s) for s in sentences) or 1
                    cur = parent_start
                    n = len(sentences)
                    for i, sent in enumerate(sentences):
                        if i == n - 1:
                            # Last span absorbs rounding so the final
                            # end_ms exactly matches the parent's end_ms.
                            end = seg.end_ms
                        else:
                            sent_chars = len(sent)
                            end = cur + round(
                                sent_chars / total_chars * parent_dur
                            )
                        spans.append(
                            UtteranceSpan(
                                start_ms=cur, end_ms=end, text=sent, words=[]
                            )
                        )
                        cur = end
                else:
                    for sent in sentences:
                        spans.append(
                            UtteranceSpan(
                                start_ms=0, end_ms=0, text=sent, words=[]
                            )
                        )
            outputs.append(UtSegOutput(source_id=item.source_id, utterances=spans))
        return outputs


__all__ = ["CHATUtteranceBackend"]
