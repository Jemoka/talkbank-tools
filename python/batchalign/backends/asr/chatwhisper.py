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

from batchalign.backends.base import ASR, UTR, BatchPolicy
from batchalign.lang import LanguageCode

# BA2 model resolution (models/resolve.py). English is the finetuned pairing;
# others fall back to base Whisper + (where present) a CHATUtterance model.
# Keyed by ISO-639-3.
_WHISPER_RESOLVE = {
    "eng": ("talkbank/CHATWhisper-en", "openai/whisper-large-v2"),
    "yue": ("alvanlii/whisper-small-cantonese", "alvanlii/whisper-small-cantonese"),
}
# Cantonese decode config (BA2 infer_asr.py): no timestamps token + DTW
# alignment heads, and generate drops task/language.
_CANTONESE_ALIGNMENT_HEADS = [
    [5, 3], [5, 9], [8, 0], [8, 4], [8, 8], [9, 0], [9, 7], [9, 9], [10, 5],
]
_UTTERANCE_RESOLVE = {
    "eng": "talkbank/CHATUtterance-en",
    "zho": "talkbank/CHATUtterance-zh_CN",
}

_ENDING_PUNCT = [".", "?", "!"]
_MOR_PUNCT = [",", "‡", "„"]
_STRIP_PUNCT = _ENDING_PUNCT + _MOR_PUNCT


# Sliding-window parameters for long-passage BERT inference. Tuned so a
# single chunk produces ~480 word-piece tokens at WordPiece-3 ratio,
# safely under BERT's 512 limit, with a 32-word overlap that the
# de-duplication logic can stitch.
_BERT_CHUNK_WORDS = 400
_BERT_CHUNK_OVERLAP = 32


def chunk_words_for_bert(
    words: list[str],
    *,
    chunk_size: int = _BERT_CHUNK_WORDS,
    overlap: int = _BERT_CHUNK_OVERLAP,
) -> list[tuple[int, list[str]]]:
    """Slice `words` into overlapping chunks for BERT inference.

    Returns a list of (start_index_in_original_words, chunk_words).
    When `len(words) <= chunk_size`, the original list is returned as a
    single chunk anchored at index 0. The overlap exists so the
    classifier sees enough left-context at each chunk boundary; the
    stitching step dedupes overlapping words by original index.
    """
    if chunk_size <= 0:
        raise ValueError("chunk_size must be positive")
    if overlap < 0 or overlap >= chunk_size:
        raise ValueError("overlap must be in [0, chunk_size)")
    if len(words) <= chunk_size:
        return [(0, list(words))]
    step = chunk_size - overlap
    chunks: list[tuple[int, list[str]]] = []
    start = 0
    n = len(words)
    while start < n:
        end = min(start + chunk_size, n)
        chunks.append((start, list(words[start:end])))
        if end == n:
            break
        start += step
    return chunks


class BertUtteranceModel:
    """TalkBank CHATUtterance BERT segmenter — faithful port of BA2.

    Predicts, per word, one of: normal / capitalize / +period / +question /
    +exclamation / +comma; reconstructs the passage with that punctuation;
    then `sent_tokenize`s into utterances. Deterministic (argmax), CPU-friendly,
    needs no audio — which is why it ports cleanly across environments.

    Long passages are sliced into overlapping word-chunks (`chunk_words_for_
    bert`) so the BERT 512-token window never overflows. Each chunk is
    classified independently; the per-word action choices are merged back
    using the original word index (latest-write wins inside the overlap
    region, which preserves boundary punctuation contributed by the
    chunk that owns the right context).

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

    def _infer_chunk(self, chunk: list[str]) -> list[int]:
        """Run BERT over one word-chunk; return per-word action labels."""
        import torch  # type: ignore[import-not-found]

        tokd = self.tokenizer(
            [chunk], return_tensors="pt", is_split_into_words=True
        ).to(self.device)
        res = self.model(**tokd).logits
        classified_targets = torch.argmax(res, dim=2).cpu()

        actions: list[int] = []
        prev_word_idx = None
        wids = tokd.word_ids(0)
        for indx, elem in enumerate(wids):
            if elem is None or elem == prev_word_idx:
                continue
            prev_word_idx = elem
            actions.append(int(classified_targets[0][indx]))
        return actions

    def __call__(self, passage: str) -> list[str]:
        import nltk  # type: ignore[import-not-found]
        from nltk import sent_tokenize  # type: ignore[import-not-found]

        passage = passage.lower().replace(".", "").replace(",", "")
        input_tokenized = passage.split(" ")

        # Sliding-window inference for long passages (>_BERT_CHUNK_WORDS).
        actions: list[int] = [0] * len(input_tokenized)
        for chunk_start, chunk_words in chunk_words_for_bert(input_tokenized):
            chunk_actions = self._infer_chunk(chunk_words)
            for i, act in enumerate(chunk_actions):
                idx = chunk_start + i
                if idx < len(actions):
                    # Latest write wins; the right-most chunk that
                    # covers `idx` has the best right-context.
                    actions[idx] = act

        res_toks: list[str] = []
        for i, w in enumerate(input_tokenized):
            action = actions[i]
            next_action = actions[i + 1] if i + 1 < len(actions) else 0
            will_action = bool(i < len(actions) - 1 and next_action > 0)
            if not will_action:
                if action == 1:
                    w = w[0].upper() + w[1:] if w else w
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


# Whisper sometimes calls Mandarin "Chinese" rather than pycountry's
# "Mandarin Chinese" / "Chinese"; the model's tokenizer accepts both,
# but BA2 standardized on "Chinese" — keep that override for parity.
_WHISPER_NAME_OVERRIDE = {
    "cmn": "Chinese",
    "zho": "Chinese",
}


def _whisper_language_name(lang: LanguageCode) -> str:
    """Whisper language NAME for a resolved `LanguageCode`.

    Pycountry's `.name` is the right answer for almost every language
    (HF Whisper's tokenizer recognizes them all). The override above
    keeps Mandarin/Chinese spelled "Chinese" to match BA2.
    """
    return _WHISPER_NAME_OVERRIDE.get(lang.alpha_3, lang.name)


class BertCantoneseUtteranceModel:
    """TalkBank Cantonese utterance segmenter — port of BA2.

    Different inference from the English `BertUtteranceModel`: it first splits
    the passage on Cantonese sentence-final particles, then per chunk runs the
    BERT token classifier (char-level) to restore punctuation, then sentence-
    tokenizes on `.?!`. Port of
    `batchalign2/batchalign/models/utterance/cantonese_infer.py`.
    """

    _KEYWORDS = ["呀", "啦", "喎", "嘞", "㗎喇", "囉", "㗎", "啊", "嗯"]

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
        self.max_length = 512
        self.model.eval()

    def __call__(self, passage: str) -> list[str]:
        import re as _re
        import torch  # type: ignore[import-not-found]

        passage = passage.lower()
        for ch in (".", ",", "!", "！", "？", "。", "，", "?", "（", "）", "：", "＊", "ｌ"):
            passage = passage.replace(ch, "")

        # Split on sentence-final particles.
        chunks: list[str] = []
        start = 0
        while start < len(passage):
            positions = [(k, passage.find(k, start)) for k in self._KEYWORDS]
            positions = [kp for kp in positions if kp[1] != -1]
            if positions:
                kw, pos = min(positions, key=lambda x: x[1])
                chunks.append(passage[start : pos + len(kw)])
                start = pos + len(kw)
            else:
                chunks.append(passage[start:])
                break

        final_passage: list[str] = []
        for chunk in chunks:
            tokenized_chunk = list(chunk)  # char-level for Chinese
            if not tokenized_chunk:
                continue
            tokd = self.tokenizer.batch_encode_plus(
                [tokenized_chunk], return_tensors="pt", truncation=True,
                padding=True, max_length=self.max_length, is_split_into_words=True,
            ).to(self.device)
            try:
                res = self.model(**tokd).logits
            except Exception:
                return []
            classified = torch.argmax(res, dim=2).cpu()
            res_toks: list[str] = []
            prev = None
            wids = tokd.word_ids(0)
            for indx, elem in enumerate(wids):
                if elem is None or elem == prev:
                    continue
                prev = elem
                action = classified[0][indx]
                w = tokenized_chunk[elem]
                will_action = bool(indx < len(wids) - 2 and classified[0][indx + 1] > 0)
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
            final_passage.append(self.tokenizer.convert_tokens_to_string(res_toks))

        text = " ".join(final_passage)
        endings = _re.compile(r"([.!?])")
        parts = _re.split(endings, text)
        out: list[str] = []
        for i in range(0, len(parts) - 1, 2):
            s = parts[i] + parts[i + 1]
            if s.strip():
                out.append(s)
        if len(parts) % 2 != 0 and parts[-1].strip():
            out.append(parts[-1].strip())
        return out


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
        return "-".join(out)
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


class ChatWhisperBackend(ASR, UTR):
    """TalkBank CHATWhisper ASR + BERT utterance segmentation; also serves `Task.Utr`."""

    def __init__(
        self,
        *,
        language: LanguageCode,
        device: str | None = None,
        batch_size: int = 1,
        batch_window_ms: int = 0,
    ) -> None:
        import torch  # type: ignore[import-not-found]  # noqa: F401
        from transformers import (  # type: ignore[import-not-found]
            GenerationConfig,
            WhisperTokenizer,
            pipeline,
        )

        from batchalign.backends.asr._torch_audio import disable_torchcodec

        disable_torchcodec()
        # Model dispatch is keyed on alpha_3 (TalkBank's finetuned
        # CHATWhisper checkpoints are language-specific); the per-call
        # `language=` kwarg uses the Whisper-style English name.
        model, base = _WHISPER_RESOLVE.get(
            language.alpha_3,
            ("openai/whisper-large-v3", "openai/whisper-large-v3"),
        )
        self._lang = language.alpha_3
        self._model_id = model

        # BA2 forces the transcription language by NAME (eng → "English");
        # without it Whisper auto-detects and can decode garbage.
        self._whisper_lang = _whisper_language_name(language)
        # BA2 decode config (infer_asr.py): discourage repetition, cache on.
        self._cantonese = language.alpha_3 == "yue"
        config = GenerationConfig.from_pretrained(base)
        config.no_repeat_ngram_size = 4
        config.use_cache = True
        if self._cantonese:
            # BA2's Cantonese branch sets these on the generation config.
            config.no_timestamps_token_id = 50363
            config.alignment_heads = _CANTONESE_ALIGNMENT_HEADS
        self._gen_config = config

        # Mirror BA2's WhisperASRModel pipeline construction exactly
        # (`models/whisper/infer_asr.py`): segment-level `return_timestamps=True`
        # (NOT word-level — word timestamps hang/garble on the finetuned
        # CHATWhisper model), the upstream `base` *tokenizer only* (the finetuned
        # repo ships no tokenizer vocab), and NO feature-extractor override (the
        # model's own extractor is correct; overriding it produced garbage).
        # bfloat16 with a float16 fallback — the dtype affects greedy decode.
        pipe_kwargs: dict[str, Any] = {
            "tokenizer": WhisperTokenizer.from_pretrained(base),
            "chunk_length_s": 25,
            "stride_length_s": 3,
            "return_timestamps": True,
        }
        if device is not None:
            pipe_kwargs["device"] = device
        try:
            self._pipe = pipeline(
                "automatic-speech-recognition", model=model,
                torch_dtype=torch.bfloat16, **pipe_kwargs,
            )
        except (TypeError, RuntimeError):
            self._pipe = pipeline(
                "automatic-speech-recognition", model=model,
                torch_dtype=torch.float16, **pipe_kwargs,
            )
        self._policy = BatchPolicy(max_size=batch_size, window_ms=batch_window_ms)

    @property
    def name(self) -> str:
        # Bump the tag when decode config changes (cache key). v3: mirror BA2
        # exactly — base WhisperTokenizer object, NO feature-extractor override,
        # segment-level return_timestamps=True (word-level hung/garbled).
        return f"chatwhisper:{self._model_id}:v5"

    @property
    def batch_policy(self) -> BatchPolicy:
        return self._policy

    def call(self, batch: list[Any], *, progress: Any = None, **_kwargs: Any) -> list[Any]:
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

        wave = np.frombuffer(item.audio.pcm_f32le, dtype=np.float32).copy()
        # BA2's Cantonese branch omits task/language in generate_kwargs.
        gen_kwargs: dict[str, Any] = {
            "generation_config": self._gen_config,
            "repetition_penalty": 1.001,
        }
        if not self._cantonese:
            gen_kwargs["task"] = "transcribe"
            gen_kwargs["language"] = self._whisper_lang
        # Segment-level timestamps (BA2 uses `return_timestamps=True`; the
        # word-level mode breaks decoding for the finetuned CHATWhisper model).
        # We only need the TEXT — the CHATUtterance UtSeg stage segments it and
        # applies disfluency/retrace, exactly as BA2 pairs its BERT segmenter to
        # the ASR output.
        result = self._pipe(
            {"array": wave, "sampling_rate": item.audio.sample_rate},
            return_timestamps=True,
            generate_kwargs=gen_kwargs,
        )

        text = (result.get("text") or "").strip()
        # BA2's `process_generation` drops word-attached punctuation from the
        # raw Whisper output (its `%mor`-free ASR blob carries no commas or
        # mid-sentence periods); the CHATUtterance BERT stage re-segments and
        # re-punctuates. Strip it here so the blob parses as a single CHAT
        # utterance before UtSeg splits it (a `.` mid-text would be an illegal
        # second terminator).
        for _p in (".", ",", "?", "!", ";", ":", '"', "„", "“", "”", "‡", "«", "»"):
            text = text.replace(_p, " ")
        text = " ".join(text.split())
        if not text:
            return AsrOutput(source_id=item.source_id, segments=[])

        chunks = result.get("chunks") or []
        start_ms = 0
        end_ms = 0
        if chunks:
            first_ts = chunks[0].get("timestamp") or (0.0, 0.0)
            last_ts = chunks[-1].get("timestamp") or (0.0, 0.0)
            start_ms = int((first_ts[0] or 0.0) * 1000)
            end_ms = int((last_ts[1] or first_ts[0] or 0.0) * 1000)

        segment = AsrSegment(
            start_ms=start_ms,
            end_ms=end_ms,
            text=text,
            speaker=None,
            words=[],
        )
        return AsrOutput(source_id=item.source_id, segments=[segment])


__all__ = [
    "ChatWhisperBackend",
    "BertUtteranceModel",
    "BertCantoneseUtteranceModel",
    "segment_words",
    "num2words_en",
]
