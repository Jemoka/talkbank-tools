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

import functools
import os
import pathlib
from typing import Any

from batchalign.backends.base import UtSeg, BatchPolicy

# BA2 model resolution for the utterance segmenter (models/resolve.py).
_UTTERANCE_RESOLVE = {
    "eng": "talkbank/CHATUtterance-en",
    "en": "talkbank/CHATUtterance-en",
    "zho": "talkbank/CHATUtterance-zh_CN",
    "zh": "talkbank/CHATUtterance-zh_CN",
    "zh-hans": "talkbank/CHATUtterance-zh_CN",
    "yue": "PolyU-AngelChanLab/Cantonese-Utterance-Segmentation",
}

_ENDING_PUNCT = (".", "?", "!")

# CHATUtterance language → BA2 support-file suffix (filled_pauses.<suffix>).
_SUPPORT_SUFFIX = {"eng": "eng", "en": "eng", "zho": "zho", "zh": "zho"}


@functools.lru_cache(maxsize=8)
def _load_cleanup(suffix: str) -> dict[str, str]:
    """Load BA2's `filled_pauses.<suffix>` + `replacements.<suffix>` into a
    `{original_lower: main_line_form}` map. Filled pauses carry their `&-`
    prefix in the main-line column (`uh` → `&-uh`); replacements map to their
    main-line form (`cuz` → `(be)cause`). Empty if no support files ship."""
    out: dict[str, str] = {}
    base = pathlib.Path(__file__).parent / "support"
    for name in (f"filled_pauses.{suffix}", f"replacements.{suffix}"):
        path = base / name
        if not path.is_file():
            continue
        for line in path.read_text().splitlines():
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            parts = line.split(" ")
            if len(parts) >= 2:
                out[parts[0].lower()] = parts[1]
    return out


def _apply_cleanup(sentence: str, table: dict[str, str]) -> str:
    """Word-level disfluency / replacement substitution (BA2 `_mark_utterance`)."""
    if not table:
        return sentence
    out_words = []
    for word in sentence.split(" "):
        out_words.append(table.get(word.lower(), word))
    return " ".join(out_words)


class CHATUtteranceBackend(UtSeg):
    """BERT utterance segmentation (BA2 `CHATUtterance`)."""

    def __init__(
        self,
        *,
        lang: str = "eng",
        batch_size: int = 8,
        batch_window_ms: int = 50,
    ) -> None:
        from batchalign.backends.asr.chatwhisper import BertUtteranceModel

        model = _UTTERANCE_RESOLVE.get(lang)
        if model is None:
            raise ValueError(
                f"no CHATUtterance segmentation model for language {lang!r}; "
                f"known: {sorted(_UTTERANCE_RESOLVE)}"
            )
        self._lang = lang
        self._model_id = model
        self._segmenter = BertUtteranceModel(model)
        # Disfluency / replacement table for this language (BA2 pairs the
        # disfluency stage with utterance segmentation in the ASR pipeline).
        self._cleanup = _load_cleanup(_SUPPORT_SUFFIX.get(lang, lang))
        self._policy = BatchPolicy(max_size=batch_size, window_ms=batch_window_ms)

    @property
    def name(self) -> str:
        # Bump the trailing tag when segmentation/cleanup behaviour changes so
        # the result cache invalidates (`disfl1` = filled-pause/replacement
        # marking added).
        return f"chatutterance:{self._model_id}:disfl1"

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
                    # Mark filled pauses / replacements (uh → &-uh) BA2-style.
                    sentence = _apply_cleanup(sentence, self._cleanup)
                    # Keep the BERT-predicted terminator on the span text; the
                    # runner reads it off the trailing punctuation so questions
                    # / exclamations survive (BA2 parity).
                    spans.append(
                        UtteranceSpan(start_ms=0, end_ms=0, text=sentence, words=[])
                    )
            outputs.append(UtSegOutput(source_id=item.source_id, utterances=spans))
        return outputs


__all__ = ["CHATUtteranceBackend"]
