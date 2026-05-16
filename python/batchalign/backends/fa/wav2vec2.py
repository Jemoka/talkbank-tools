"""Wav2Vec2FaBackend: forced alignment via Wav2Vec2 CTC.

Mirrors BA2's wav2vec2 alignment (``batchalign2/batchalign/pipelines/fa/``).
We run a CTC-trained Wav2Vec2 model over the audio, then align the
provided utterance transcript against the per-frame log-probabilities
using the standard Viterbi forced-alignment recurrence.

The defaults match BA2's defaults:

* English:  ``facebook/wav2vec2-large-960h``
* Multi:    ``facebook/wav2vec2-large-xlsr-53``

Switch via ``model=`` or one of the per-language helpers in
:func:`model_for_language`.

The aligner is fed transcript text per utterance from
``FaInput.utterances``; output is a parallel list of :class:`AsrSegment`
where ``words[*].start_ms`` / ``end_ms`` are populated from the CTC
alignment. Utterance-level ``start_ms`` / ``end_ms`` come from the
first / last word.
"""

from __future__ import annotations

import re
from typing import Any

from batchalign.backends.base import FA, BatchPolicy


_LANG_DEFAULTS = {
    "eng": "facebook/wav2vec2-large-960h",
    "en": "facebook/wav2vec2-large-960h",
    "zh": "jonatasgrosman/wav2vec2-large-xlsr-53-chinese-zh-cn",
    "yue": "scottykwok/wav2vec2-large-xlsr-cantonese",
    "fra": "facebook/wav2vec2-large-xlsr-53-french",
    "fr": "facebook/wav2vec2-large-xlsr-53-french",
    "spa": "facebook/wav2vec2-large-xlsr-53-spanish",
    "es": "facebook/wav2vec2-large-xlsr-53-spanish",
    "deu": "facebook/wav2vec2-large-xlsr-53-german",
    "de": "facebook/wav2vec2-large-xlsr-53-german",
    "jpn": "jonatasgrosman/wav2vec2-large-xlsr-53-japanese",
    "ja": "jonatasgrosman/wav2vec2-large-xlsr-53-japanese",
}


def model_for_language(lang: str | None) -> str:
    """Return the default wav2vec2 model id for a language, fall back to English."""
    if not lang:
        return _LANG_DEFAULTS["eng"]
    return _LANG_DEFAULTS.get(lang.lower(), "facebook/wav2vec2-large-xlsr-53")


# Strip CHAT-style markup so the aligner sees only spoken tokens.
_CHAT_STRIP_RE = re.compile(r"[\[\]()<>{}@#$%\*&]")
_PUNCT_RE = re.compile(r"[.!?,;:\"]+")


def _normalize_for_ctc(text: str) -> str:
    """Strip annotation symbols and lowercase for CTC vocab matching."""
    cleaned = _CHAT_STRIP_RE.sub(" ", text)
    cleaned = _PUNCT_RE.sub(" ", cleaned)
    return " ".join(cleaned.lower().split())


class Wav2Vec2FaBackend(FA):
    """Forced alignment using Wav2Vec2 + CTC Viterbi.

    Constructor knobs:

    * ``model``: HuggingFace model id. If ``None`` we pick from
      :func:`model_for_language` at ``call``-time per utterance's
      language hint.
    * ``device``: torch device string (``"cpu"`` / ``"cuda"`` / ``"mps"``).
    * ``batch_size``: forwarded to :class:`BatchPolicy`.
    """

    def __init__(
        self,
        model: str | None = None,
        *,
        device: str | None = None,
        batch_size: int = 4,
        batch_window_ms: int = 100,
    ) -> None:
        import torch  # type: ignore[import-not-found]
        from transformers import (  # type: ignore[import-not-found]
            Wav2Vec2ForCTC,
            Wav2Vec2Processor,
        )

        chosen = model or _LANG_DEFAULTS["eng"]
        self._processor = Wav2Vec2Processor.from_pretrained(chosen)
        self._model = Wav2Vec2ForCTC.from_pretrained(chosen)
        self._device = torch.device(device) if device else torch.device("cpu")
        self._model = self._model.to(self._device)
        self._model.eval()
        self._model_id = chosen
        self._policy = BatchPolicy(max_size=batch_size, window_ms=batch_window_ms)

    @property
    def name(self) -> str:
        return f"wav2vec2-fa:{self._model_id}"

    @property
    def batch_policy(self) -> BatchPolicy:
        return self._policy

    def call(self, batch: list[Any]) -> list[Any]:
        from batchalign._core.proto import FaInput, FaOutput

        outputs: list[Any] = []
        for item in batch:
            if not isinstance(item, FaInput):
                raise TypeError(
                    f"Wav2Vec2FaBackend does not handle input type: {type(item).__name__}"
                )
            outputs.append(self._align_one(item, FaOutput))
        return outputs

    # ----- internals -----------------------------------------------------

    def _align_one(self, item: Any, FaOutput: type) -> Any:
        """Align every utterance in ``item.utterances`` against ``item.audio``.

        Strategy: run the full audio through wav2vec2 once to get per-frame
        log-probabilities (``emission``), then for each utterance build a
        token sequence and run Viterbi over the slice of the emission
        bounded by its current ``[start_ms, end_ms]``.
        """
        import numpy as np  # type: ignore[import-not-found]
        import torch  # type: ignore[import-not-found]
        from batchalign._core.proto import AsrSegment, AsrWord

        if not item.utterances:
            return FaOutput(source_id=item.source_id, utterances=[])

        wave = np.frombuffer(item.audio.pcm_f32le, dtype=np.float32)
        sr = int(item.audio.sample_rate)

        inputs = self._processor(
            wave, sampling_rate=sr, return_tensors="pt", padding=True
        )
        input_values = inputs.input_values.to(self._device)
        with torch.no_grad():
            logits = self._model(input_values).logits[0]
        emission = torch.log_softmax(logits, dim=-1).cpu()
        n_frames = emission.shape[0]
        total_samples = wave.shape[0]
        frame_to_ms = (total_samples / sr) * 1000.0 / max(n_frames, 1)

        vocab = self._processor.tokenizer.get_vocab()
        # CTC blank is the "<pad>" id by convention for HF wav2vec2.
        blank_id = self._processor.tokenizer.pad_token_id or 0

        aligned: list[Any] = []
        for utt in item.utterances:
            start_ms = int(getattr(utt, "start_ms", 0) or 0)
            end_ms = int(getattr(utt, "end_ms", 0) or 0)
            if end_ms <= start_ms:
                end_ms = int(total_samples / sr * 1000)
            f_lo = max(int(start_ms / max(frame_to_ms, 1e-9)), 0)
            f_hi = min(int(end_ms / max(frame_to_ms, 1e-9)) + 1, n_frames)
            if f_hi <= f_lo:
                aligned.append(
                    AsrSegment(
                        start_ms=start_ms,
                        end_ms=end_ms,
                        text=utt.text,
                        speaker=getattr(utt, "speaker", None),
                        words=[],
                    )
                )
                continue
            slice_emission = emission[f_lo:f_hi].numpy()

            words = [w.text for w in (utt.words or [])] or _normalize_for_ctc(
                utt.text
            ).split()
            if not words:
                aligned.append(
                    AsrSegment(
                        start_ms=start_ms,
                        end_ms=end_ms,
                        text=utt.text,
                        speaker=getattr(utt, "speaker", None),
                        words=[],
                    )
                )
                continue

            word_spans = _ctc_word_align(
                slice_emission, words, vocab, blank_id
            )
            aligned_words = []
            for (w, (fs, fe)) in zip(words, word_spans):
                ws_ms = int(start_ms + fs * frame_to_ms)
                we_ms = int(start_ms + fe * frame_to_ms)
                aligned_words.append(
                    AsrWord(text=w, start_ms=ws_ms, end_ms=we_ms, confidence=None)
                )
            aligned.append(
                AsrSegment(
                    start_ms=aligned_words[0].start_ms,
                    end_ms=aligned_words[-1].end_ms,
                    text=utt.text,
                    speaker=getattr(utt, "speaker", None),
                    words=aligned_words,
                )
            )

        return FaOutput(source_id=item.source_id, utterances=aligned)


def _ctc_word_align(
    emission: Any,
    words: list[str],
    vocab: dict[str, int],
    blank_id: int,
) -> list[tuple[int, int]]:
    """Viterbi-align a sequence of words against a CTC emission slice.

    ``emission`` is a ``(frames, vocab)`` log-softmax numpy array.
    Returns per-word ``(frame_start, frame_end)`` (inclusive lo, exclusive hi).

    The implementation follows the standard "frame-by-frame greedy + token
    spans" approach used by torchaudio's forced-alignment tutorial: build
    a flat token sequence with word-separator boundaries, run a token-by-
    token DP, then translate token spans back into word spans.
    """
    import numpy as np  # type: ignore[import-not-found]

    # Build flat list of token ids, remembering which word each token belongs to.
    word_sep = vocab.get("|", vocab.get(" ", blank_id))
    tokens: list[int] = []
    owner: list[int] = []  # owner[i] = word index for tokens[i]
    for wi, w in enumerate(words):
        if wi > 0:
            tokens.append(word_sep)
            owner.append(-1)
        for ch in w:
            tid = vocab.get(ch.upper(), vocab.get(ch.lower(), blank_id))
            tokens.append(tid)
            owner.append(wi)
    if not tokens:
        return [(0, emission.shape[0]) for _ in words]

    n_frames = emission.shape[0]
    n_tokens = len(tokens)

    # Trellis: trellis[t, j] = best log-prob of using up to frame t to
    # complete j tokens.
    neg_inf = -1e9
    trellis = np.full((n_frames + 1, n_tokens + 1), neg_inf, dtype=np.float32)
    trellis[:, 0] = 0.0
    for t in range(n_frames):
        for j in range(1, min(t + 1, n_tokens) + 1):
            stay = trellis[t, j] + emission[t, blank_id]
            advance = trellis[t, j - 1] + emission[t, tokens[j - 1]]
            trellis[t + 1, j] = max(stay, advance)

    # Backtrack: at each frame decide if we stayed or advanced.
    t, j = n_frames, n_tokens
    token_frames: list[list[int]] = [[] for _ in range(n_tokens)]
    while j > 0 and t > 0:
        # Decide whether the optimum came from stay or advance.
        stay = trellis[t - 1, j] + emission[t - 1, blank_id]
        advance = trellis[t - 1, j - 1] + emission[t - 1, tokens[j - 1]]
        if advance >= stay:
            token_frames[j - 1].append(t - 1)
            j -= 1
        t -= 1

    # Word frame spans from owner mapping.
    word_spans: list[tuple[int, int]] = []
    for wi in range(len(words)):
        frames: list[int] = []
        for ti, w_idx in enumerate(owner):
            if w_idx == wi:
                frames.extend(token_frames[ti])
        if frames:
            word_spans.append((min(frames), max(frames) + 1))
        else:
            # Fallback: spread evenly if the trellis collapsed.
            chunk = max(n_frames // max(len(words), 1), 1)
            word_spans.append((wi * chunk, (wi + 1) * chunk))
    return word_spans


__all__ = ["Wav2Vec2FaBackend", "model_for_language"]
