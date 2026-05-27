"""ChatWhisperBackend: BA2's TalkBank CHATWhisper ASR + BERT utterance segmenter.

This is the BA3 equivalent of BA2's `WhisperEngine`
(`batchalign2/batchalign/pipelines/asr/whisper.py`), which is what `parity.md`
means by "openai's implementation of whisper" wired for CHAT: it pairs the
TalkBank-finetuned Whisper model (`talkbank/CHATWhisper-en`) with the
TalkBank utterance-segmentation model (`talkbank/CHATUtterance-en`,
a `BertForTokenClassification` that predicts sentence boundaries +
capitalization + punctuation). The utterance segmentation is the parity-
critical part — it is what makes BA3's per-utterance segmentation match BA2's,
which a plain Whisper pass cannot reproduce.

It is a normal `ASR` backend (no new backend *type*): it emits one
`AsrSegment` per **utterance** (already segmented by the BERT model), so the
ASR runner's utterances are BA2's utterances.

Design notes / parity caveats:
  - ASR uses the modern `transformers` pipeline with `return_timestamps="word"`
    and BA2's decode config (`no_repeat_ngram_size=4`, `repetition_penalty`),
    rather than BA2's transformers-4.x DTW monkeypatch (incompatible with
    transformers 5.x). The decoded TEXT is what drives segmentation parity;
    exact word-timestamp parity is out of scope (timings are cosmetic for the
    `transcribe` important lines — segmentation + text).
  - The deterministic segmentation glue (`segment_words`) and the BERT model
    wrapper are factored out so they unit-test without the heavy ASR model.

Models are pulled from HuggingFace (resolve table below), matching BA2's
`models/resolve.py`.
"""

from __future__ import annotations

import re
from typing import Any

from batchalign.backends.base import ASR, BatchPolicy

# BA2 model resolution (models/resolve.py). English is the finetuned pairing;
# others fall back to base Whisper + (where present) a CHATUtterance model.
_WHISPER_RESOLVE = {
    "eng": ("talkbank/CHATWhisper-en", "openai/whisper-large-v2"),
    "en": ("talkbank/CHATWhisper-en", "openai/whisper-large-v2"),
}
_UTTERANCE_RESOLVE = {
    "eng": "talkbank/CHATUtterance-en",
    "en": "talkbank/CHATUtterance-en",
    "zho": "talkbank/CHATUtterance-zh_CN",
    "zh": "talkbank/CHATUtterance-zh_CN",
}

_ENDING_PUNCT = [".", "?", "!"]
_MOR_PUNCT = [",", "‡", "„"]
_STRIP_PUNCT = _ENDING_PUNCT + _MOR_PUNCT


class BertUtteranceModel:
    """TalkBank CHATUtterance BERT segmenter — faithful port of BA2.

    Predicts, per word, one of: normal / capitalize / +period / +question /
    +exclamation / +comma; reconstructs the passage with that punctuation;
    then `sent_tokenize`s into utterances. Deterministic (argmax), CPU-friendly,
    needs no audio — which is why it ports cleanly across environments.

    Port of `batchalign2/batchalign/models/utterance/infer.py`.
    """

    def __init__(self, model: str) -> None:
        import torch  # type: ignore[import-not-found]
        from transformers import (  # type: ignore[import-not-found]
            AutoTokenizer,
            BertForTokenClassification,
        )

        device = torch.device("cuda") if torch.cuda.is_available() else torch.device("cpu")
        self.tokenizer = AutoTokenizer.from_pretrained(model)
        self.model = BertForTokenClassification.from_pretrained(model).to(device)
        self.device = device
        self.model.eval()

    def __call__(self, passage: str) -> list[str]:
        import torch  # type: ignore[import-not-found]
        import nltk  # type: ignore[import-not-found]
        from nltk import sent_tokenize  # type: ignore[import-not-found]

        passage = passage.lower().replace(".", "").replace(",", "")
        input_tokenized = passage.split(" ")

        tokd = self.tokenizer(
            [input_tokenized], return_tensors="pt", is_split_into_words=True
        ).to(self.device)
        res = self.model(**tokd).logits
        classified_targets = torch.argmax(res, dim=2).cpu()

        res_toks: list[str] = []
        prev_word_idx = None
        wids = tokd.word_ids(0)
        for indx, elem in enumerate(wids):
            if elem is None or elem == prev_word_idx:
                continue
            prev_word_idx = elem
            action = classified_targets[0][indx]
            w = input_tokenized[elem]

            will_action = bool(indx < len(wids) - 2 and classified_targets[0][indx + 1] > 0)
            if not will_action:
                if action == 1:
                    w = w[0].upper() + w[1:]
                elif action == 2:
                    w = w + "."
                elif action == 3:
                    w = w + "?"
                elif action == 4:
                    w = w + "!"
                elif action == 5:
                    w = w + ","
            res_toks.append(w)

        final_passage = self.tokenizer.convert_tokens_to_string(res_toks)
        try:
            return sent_tokenize(final_passage)
        except LookupError:
            nltk.download("punkt")
            nltk.download("punkt_tab")
            return sent_tokenize(final_passage)


def num2words_en(word: str) -> str:
    """English number-word expansion (BA2 `catched_num2words`, English path)."""
    word = re.sub(r"[—–]", "-", word)
    parts = word.split("-")
    if len(parts) > 1:
        out = []
        for p in parts:
            try:
                out.append(num2words_en(p))
            except NotImplementedError:
                return word
        return " ".join(out)
    if not word.isdigit():
        return word
    try:
        from num2words import num2words as _n2w  # type: ignore[import-not-found]

        return _n2w(word, lang="en")
    except Exception:
        return word


def segment_words(
    words: list[tuple[str, int | None, int | None]],
    speaker: Any,
    segmenter: Any,
) -> list[tuple[str, list[tuple[str, int | None, int | None]], str]]:
    """Split a flat word stream into utterances via the BERT segmenter.

    Faithful port of BA2's `retokenize_with_engine` (+ the relevant
    `process_generation` word handling). `words` is `[(text, start_ms,
    end_ms)]`. Returns a list of `(speaker, utterance_words, terminator)` where
    `utterance_words` is the slice of input words for that utterance and
    `terminator` is the sentence-final mark the segmenter chose (`.`/`?`/`!`).

    Pure given the segmenter — unit-tested with a fake segmenter.
    """
    # Strip preexisting punctuation + lowercase (BA2 feeds the segmenter clean).
    cleaned: list[tuple[str, int | None, int | None]] = []
    for text, s, e in words:
        t = text
        for p in _STRIP_PUNCT:
            t = t.strip(p)
        t = t.lower()
        if t.strip():
            cleaned.append((t, s, e))
    if not cleaned:
        return []

    joined = " ".join(t for t, _, _ in cleaned).replace("。", ".")
    sentences = segmenter(joined)

    out: list[tuple[str, list[tuple[str, int | None, int | None]], str]] = []
    idx = 0
    for sent in sentences:
        if sent and sent[-1] in _ENDING_PUNCT:
            toks, delim = sent[:-1].split(" "), sent[-1]
        else:
            toks, delim = sent.split(" "), "."
        utt: list[tuple[str, int | None, int | None]] = []
        for tok in toks:
            if not tok:
                continue
            if idx < len(cleaned):
                # Carry the original (timed) word, but use the segmenter's
                # casing/spelling for the surface form.
                _, s, e = cleaned[idx]
                utt.append((tok, s, e))
                idx += 1
            else:
                utt.append((tok, None, None))
        if utt:
            out.append((speaker, utt, delim))
    return out


class ChatWhisperBackend(ASR):
    """TalkBank CHATWhisper ASR + BERT utterance segmentation (BA2 WhisperEngine)."""

    def __init__(
        self,
        *,
        lang: str = "eng",
        device: str | None = None,
        batch_size: int = 1,
        batch_window_ms: int = 0,
    ) -> None:
        import torch  # type: ignore[import-not-found]  # noqa: F401
        from transformers import (  # type: ignore[import-not-found]
            GenerationConfig,
            pipeline,
        )

        from batchalign.backends.asr._torch_audio import disable_torchcodec

        disable_torchcodec()
        model, base = _WHISPER_RESOLVE.get(lang, ("openai/whisper-large-v3", "openai/whisper-large-v3"))
        self._lang = lang
        self._model_id = model

        # BA2 decode config (infer_asr.py): discourage repetition, cache on.
        config = GenerationConfig.from_pretrained(base)
        config.no_repeat_ngram_size = 4
        config.use_cache = True
        self._gen_config = config

        pipe_kwargs: dict[str, Any] = {
            "chunk_length_s": 25,
            "stride_length_s": 3,
            "return_timestamps": "word",
        }
        if device is not None:
            pipe_kwargs["device"] = device
        self._pipe = pipeline("automatic-speech-recognition", model=model, **pipe_kwargs)
        self._policy = BatchPolicy(max_size=batch_size, window_ms=batch_window_ms)

    @property
    def name(self) -> str:
        return f"chatwhisper:{self._model_id}"

    @property
    def batch_policy(self) -> BatchPolicy:
        return self._policy

    def call(self, batch: list[Any]) -> list[Any]:
        from batchalign._core.proto import AsrInput, AsrOutput

        outputs: list[Any] = []
        for item in batch:
            if not isinstance(item, AsrInput):
                raise TypeError(f"ChatWhisperBackend does not handle: {type(item).__name__}")
            outputs.append(self._transcribe(item, AsrOutput))
        return outputs

    def _transcribe(self, item: Any, AsrOutput: type) -> Any:
        import numpy as np  # type: ignore[import-not-found]
        from batchalign._core.proto import AsrSegment, AsrWord

        wave = np.frombuffer(item.audio.pcm_f32le, dtype=np.float32)
        result = self._pipe(
            {"array": wave, "sampling_rate": item.audio.sample_rate},
            return_timestamps="word",
            generate_kwargs={
                "generation_config": self._gen_config,
                "repetition_penalty": 1.001,
                "task": "transcribe",
            },
        )

        # Flatten to (text, start_ms, end_ms), expanding number words, dropping
        # empties and pseudo-tokens (BA2 process_generation).
        words: list[tuple[str, int | None, int | None]] = []
        for chunk in result.get("chunks", []):
            txt = (chunk.get("text") or "").strip()
            if not txt or re.match(r"<.*>", txt):
                continue
            ts = chunk.get("timestamp") or (None, None)
            s = int(ts[0] * 1000) if ts[0] is not None else None
            e = int(ts[1] * 1000) if ts[1] is not None else s
            words.append((num2words_en(txt), s, e))

        if not words:
            return AsrOutput(source_id=item.source_id, segments=[])

        # Emit ONE raw segment (the whole monologue). Utterance segmentation +
        # disfluency/retrace cleanup is the CHATUtterance UtSeg stage's job (the
        # transcribe recipe pairs it), exactly as BA2 applies its BERT segmenter
        # to the ASR word stream. This unifies chatwhisper with the other ASR
        # engines instead of segmenting internally without cleanup.
        timed = [w for w in words if w[1] is not None]
        start_ms = timed[0][1] if timed else 0
        end_ms = timed[-1][2] if timed else start_ms
        asr_words = [
            AsrWord(text=t, start_ms=s or 0, end_ms=e or (s or 0), confidence=None)
            for (t, s, e) in words
        ]
        segment = AsrSegment(
            start_ms=start_ms,
            end_ms=end_ms,
            text=" ".join(t for t, _, _ in words),
            speaker=None,
            words=asr_words,
        )
        return AsrOutput(source_id=item.source_id, segments=[segment])


__all__ = ["ChatWhisperBackend", "BertUtteranceModel", "segment_words", "num2words_en"]
