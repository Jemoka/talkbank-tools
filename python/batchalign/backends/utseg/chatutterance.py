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
        # Cantonese uses a distinct inference (particle-chunking) model.
        if lang == "yue":
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
        # the result cache invalidates (`disfl2` = + retrace [/] marking).
        return f"chatutterance:{self._model_id}:disfl2"

    @property
    def batch_policy(self) -> BatchPolicy:
        return self._policy

    def call(self, batch: list[Any]) -> list[Any]:
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
                for sentence in self._segmenter(text):
                    sentence = sentence.strip()
                    if not sentence:
                        continue
                    # BA2 pairing: disfluency (uh → &-uh) then retrace ([/]).
                    sentence = clean_utterance(sentence, self._cleanup, self._lang)
                    # Keep the BERT-predicted terminator on the span text; the
                    # runner reads it off the trailing punctuation so questions
                    # / exclamations survive (BA2 parity).
                    spans.append(
                        UtteranceSpan(start_ms=0, end_ms=0, text=sentence, words=[])
                    )
            outputs.append(UtSegOutput(source_id=item.source_id, utterances=spans))
        return outputs


__all__ = ["CHATUtteranceBackend"]
