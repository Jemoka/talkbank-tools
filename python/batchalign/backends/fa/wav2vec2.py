"""Wav2Vec2FaBackend: forced alignment via torchaudio MMS_FA (BA2 parity).

Faithful port of BA2's wav2vec forced aligner
(`batchalign2/batchalign/models/wave2vec/infer_fa.py` +
`pipelines/fa/wave2vec_fa.py`). BA2's `--wav2vec` FA is **MMS_FA**
(`torchaudio.pipelines.MMS_FA`) + `forced_align` / `merge_tokens`, not a
per-language CTC model — so we use the same to make the `%wor` tier match.

Algorithm (per BA2):
  1. Group the utterances into ≤15 s chunks by their media-bullet windows.
  2. For each chunk, slice the audio to `[group_start, group_end]`, build the
     character transcript (punctuation stripped, lower-cased), run MMS_FA
     `forced_align` → per-word spans in ms (relative to the chunk start).
  3. Char-level DP-align the MMS_FA output words back to the source words
     (handles punctuation/markup mismatches) and set each word's timing
     `+= group_start`.
  4. Post-correct: keep the FA-derived end (extend by ~500 ms only when the
     next item is untimed — e.g. the terminal punctuation after the last word),
     then bound the span by the utterance window; drop impossible spans.

The DP aligner is BA2's (`backends/morphosyntax/ud/dp.py`, copied verbatim).
"""

from __future__ import annotations

from typing import Any

from batchalign.backends.base import FA, BatchPolicy
from batchalign.backends.morphosyntax.ud.dp import (
    Match,
    PayloadTarget,
    ReferenceTarget,
    align,
)

# Punctuation stripped from the transcript before alignment (BA2 MOR/ENDING).
_STRIP = {".", "?", "!", ",", ";", ":", "‡", "„", '"', "+", "<", ">", "[", "]", "/"}
_GROUP_MS = 15 * 1000  # BA2 groups utterances into ~15 s chunks.


def model_for_language(lang: str | None) -> str:
    """Kept for API compatibility; MMS_FA is language-agnostic."""
    return "torchaudio:MMS_FA"


def _strip_word(text: str) -> str:
    """Drop punctuation/markup so MMS_FA sees only alignable characters."""
    return "".join(c for c in text if c not in _STRIP).strip()


class Wav2Vec2FaBackend(FA):
    """Forced alignment via torchaudio MMS_FA (BA2's `--wav2vec`)."""

    def __init__(
        self,
        model: str | None = None,
        *,
        device: str | None = None,
        batch_size: int = 2,
        batch_window_ms: int = 0,
    ) -> None:
        import torch  # type: ignore[import-not-found]
        import torchaudio  # type: ignore[import-not-found]

        self._bundle = torchaudio.pipelines.MMS_FA
        self._model = self._bundle.get_model()
        self._dict = self._bundle.get_dict()
        self._device = torch.device(device) if device else torch.device("cpu")
        self._model = self._model.to(self._device)
        self._model.eval()
        self._sr = 16000
        self._policy = BatchPolicy(max_size=batch_size, window_ms=batch_window_ms)

    @property
    def name(self) -> str:
        # Bump when the alignment/post-correction changes (cache key).
        return "wav2vec2-fa:mms_fa-v3"

    @property
    def batch_policy(self) -> BatchPolicy:
        return self._policy

    def call(
        self, batch: list[Any], *, progress: Any = None, **_kwargs: Any
    ) -> list[Any]:
        from batchalign._core.proto import FaInput, FaOutput

        # `progress(completed, total)` is forwarded into `_align_one` so
        # the per-audio-group loop can tick. Per the ABC, this is the
        # meaningful intra-call granularity for FA (chunks ≤ _GROUP_MS).
        outputs: list[Any] = []
        for item in batch:
            if not isinstance(item, FaInput):
                raise TypeError(
                    f"Wav2Vec2FaBackend does not handle input type: {type(item).__name__}"
                )
            outputs.append(self._align_one(item, FaOutput, progress=progress))
        return outputs

    # ----- internals -----------------------------------------------------

    def _mms(self, audio_chunk: Any, words: list[str]) -> list[tuple[str, tuple[int, int]]]:
        """Run MMS_FA on one audio chunk + word list → `[(word, (s_ms,e_ms))]`.

        Port of BA2 `Wave2VecFAModel.__call__`. Times are relative to the chunk.
        """
        import torch  # type: ignore[import-not-found]
        import torchaudio.functional as AF  # type: ignore[import-not-found]

        audio_chunk = audio_chunk.to(self._device)
        emission, _ = self._model(audio_chunk.unsqueeze(0))
        emission = emission.cpu().detach()

        star = self._dict.get("*", 0)
        transcript = torch.tensor(
            [self._dict.get(c, star) for word in words for c in word.lower()]
        )
        if transcript.numel() == 0:
            return [(w, (0, 0)) for w in words]

        path, scores = AF.forced_align(emission, transcript.unsqueeze(0))
        alignments, scores = path[0], scores[0]
        scores = scores.exp()
        token_spans = AF.merge_tokens(alignments, scores)

        def unflatten(list_, lengths):
            i = 0
            ret = []
            for length in lengths:
                ret.append(list_[i : i + length])
                i += length
            return ret

        word_spans = unflatten(token_spans, [len(w) for w in words])
        ratio = audio_chunk.size(0) / emission.size(1)
        out = []
        for w, spans in zip(words, word_spans):
            if not spans:
                out.append((w, (0, 0)))
                continue
            s = int(((spans[0].start * ratio) / self._sr) * 1000)
            e = int(((spans[-1].end * ratio) / self._sr) * 1000)
            out.append((w, (s, e)))
        return out

    def _align_one(self, item: Any, FaOutput: type, *, progress: Any = None) -> Any:
        import numpy as np  # type: ignore[import-not-found]
        import torch  # type: ignore[import-not-found]
        from batchalign._core.proto import AsrSegment, AsrWord

        if not item.utterances:
            return FaOutput(source_id=item.source_id, utterances=[])

        import torchaudio.functional as AF  # type: ignore[import-not-found]

        wave = torch.from_numpy(
            np.frombuffer(item.audio.pcm_f32le, dtype=np.float32).copy()
        )
        src_sr = int(item.audio.sample_rate)
        # MMS_FA is a 16 kHz model. The Rust audio-prep keeps the file's
        # native rate, so resample once here — without this, time conversion
        # below (which uses `self._sr = 16000`) is off by `src_sr / 16000`.
        if src_sr != self._sr:
            wave = AF.resample(wave, src_sr, self._sr)
        sr = self._sr

        # Flatten utterance words into (uidx, widx, text), tracking each
        # utterance's window. Words carry their utterance's window time.
        # Mutable timing store: timings[uidx][widx] = (start_ms, end_ms) | None.
        timings: list[list[Any]] = [
            [None] * len(utt.words) for utt in item.utterances
        ]
        windows = [
            (int(getattr(u, "start_ms", 0) or 0), int(getattr(u, "end_ms", 0) or 0))
            for u in item.utterances
        ]

        # Build BA2-style 15 s groups of (uidx, widx) over utterances that have
        # a window. Each group aligns together against its audio span.
        groups: list[list[tuple[int, int]]] = []
        group: list[tuple[int, int]] = []
        seg_start: int | None = None
        for uidx, utt in enumerate(item.utterances):
            w0, w1 = windows[uidx]
            if w1 <= w0:
                continue  # no window → cannot place in audio; skip (no timing)
            if seg_start is None:
                seg_start = w0
            if (w1 - seg_start) > _GROUP_MS and group:
                groups.append(group)
                group = []
                seg_start = w0
            for widx in range(len(utt.words)):
                group.append((uidx, widx))
        if group:
            groups.append(group)

        n_groups = len(groups)
        for group_idx, grp in enumerate(groups):
            # Per-group progress tick — drives the per-file bar inside
            # the single bulk dispatch. Ticks AFTER the group is aligned
            # so the bar reflects completed work, not started work.
            if not grp:
                if progress is not None:
                    progress(group_idx + 1, n_groups)
                continue
            g_start = windows[grp[0][0]][0]
            g_end = windows[grp[-1][0]][1]
            lo = int(g_start * sr / 1000)
            hi = int(g_end * sr / 1000)
            if hi <= lo or hi > wave.shape[0]:
                hi = min(hi, wave.shape[0])
            if hi <= lo:
                continue
            chunk = wave[lo:hi]

            src_words = [item.utterances[u].words[w].text for (u, w) in grp]
            transcript = [_strip_word(t) for t in src_words]
            # MMS_FA needs non-empty alignable words; keep index parity by
            # substituting a single char for empties.
            transcript = [t if t else "·" for t in transcript]
            try:
                res = self._mms(chunk, transcript)
            except Exception:
                continue

            # Char-DP map MMS_FA output words back to source words (BA2).
            ref_targets = [
                ReferenceTarget(ch, payload=i)
                for i, t in enumerate(transcript)
                for ch in t
            ]
            payload_targets = []
            res_times = []
            for ri, (w, time) in enumerate(res):
                res_times.append(time)
                for ch in w:
                    payload_targets.append(PayloadTarget(ch, payload=ri))
            alignments = align(payload_targets, ref_targets, tqdm=False)
            alignments.reverse()
            for elem in alignments:
                if isinstance(elem, Match):
                    gi = elem.reference_payload
                    uidx, widx = grp[gi]
                    t = res_times[elem.payload]
                    timings[uidx][widx] = (
                        int(round(t[0] + g_start)),
                        int(round(t[1] + g_start)),
                    )
            if progress is not None:
                progress(group_idx + 1, n_groups)

        # Build output utterances, applying BA2's post-correction (bump each
        # word's end to the next word's start, bound by the utterance window).
        aligned: list[Any] = []
        for uidx, utt in enumerate(item.utterances):
            w0, w1 = windows[uidx]
            wt = timings[uidx]
            n = len(utt.words)
            words_out = []
            for widx, word in enumerate(utt.words):
                t = wt[widx]
                # BA2's wave2vec post-correction keeps the FA-derived end (it
                # does NOT bump to the next word's start); it only, when the
                # NEXT content item is untimed, extends by ~500 ms, then bounds
                # the span by the utterance window. The terminal punctuation
                # (`.`/`?`/`!`) is an untimed item after the LAST word, so the
                # last word also takes the extend-then-bound path (→ window end).
                if t is not None:
                    nxt = widx + 1
                    while nxt < n - 1 and wt[nxt] is None:
                        nxt += 1
                    next_untimed = nxt >= n or wt[nxt] is None
                    if next_untimed:
                        t = (t[0], t[0] + 500)
                if t is not None and w1 > w0:
                    t = (max(t[0], w0), min(t[1], w1))
                    if t[0] >= t[1]:
                        t = None
                if t is None:
                    words_out.append(
                        AsrWord(text=word.text, start_ms=0, end_ms=0, confidence=None)
                    )
                else:
                    words_out.append(
                        AsrWord(text=word.text, start_ms=t[0], end_ms=t[1], confidence=None)
                    )
            timed = [w for w in words_out if w.end_ms > 0]
            aligned.append(
                AsrSegment(
                    start_ms=timed[0].start_ms if timed else w0,
                    end_ms=timed[-1].end_ms if timed else w1,
                    text=utt.text,
                    speaker=getattr(utt, "speaker", None),
                    words=words_out,
                )
            )
        return FaOutput(source_id=item.source_id, utterances=aligned)


__all__ = ["Wav2Vec2FaBackend", "model_for_language"]
