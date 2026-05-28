"""WhisperFaBackend: forced alignment via Whisper cross-attention DTW (BA2 parity).

Faithful port of BA2's `--whisper_fa` aligner
(`batchalign2/batchalign/models/whisper/infer_fa.py::WhisperFAModel` +
`pipelines/fa/whisper_fa.py::WhisperFAEngine`). Unlike the wav2vec aligner
(torchaudio MMS_FA), this aligns by running the Whisper model in teacher-forcing
mode over the transcript, reading the decoder→encoder cross-attention of the
model's `alignment_heads`, and dynamic-time-warping that attention matrix to
recover per-token timestamps — exactly the method BA2 uses, so the `%wor`
timings match.

Algorithm (per BA2):
  1. Group utterances into ~20 s chunks by their media-bullet windows.
  2. For each chunk, slice the audio, build the de-punctuated transcript, and
     run the DTW aligner → per-token `(text, time_s)` relative to the chunk.
  3. Char-level DP-align the DTW tokens back to the source words (handles
     punctuation / tokenizer mismatches) and set each word's start `+= group
     start`.
  4. Post-correct: bump each word's end to the next word's start, bound by the
     utterance window; drop impossible spans.

The DP aligner is BA2's (`backends/morphosyntax/ud/dp.py`, copied verbatim).
Default model is `openai/whisper-large-v2` (BA2's default), loaded with
`attn_implementation="eager"` so cross-attentions are available.
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
_GROUP_MS = 20 * 1000  # BA2 groups utterances into ~20 s chunks for whisper.
_TIME_PRECISION = 0.02  # Whisper frame stride (seconds), BA2 infer_fa.py.


def _strip_word(text: str) -> str:
    return "".join(c for c in text if c not in _STRIP).strip()


class WhisperFaBackend(FA):
    """Forced alignment via Whisper cross-attention DTW (BA2's `--whisper_fa`)."""

    def __init__(
        self,
        model: str | None = None,
        *,
        device: str | None = None,
        batch_size: int = 1,
        batch_window_ms: int = 0,
    ) -> None:
        import torch  # type: ignore[import-not-found]
        from transformers import (  # type: ignore[import-not-found]
            WhisperForConditionalGeneration,
            WhisperProcessor,
        )

        from batchalign.backends.asr._torch_audio import disable_torchcodec

        disable_torchcodec()
        self._model_id = model or "openai/whisper-large-v2"
        self._device = torch.device(device) if device else torch.device("cpu")
        # `eager` attention is required to read `output_attentions` (BA2).
        self._model = WhisperForConditionalGeneration.from_pretrained(
            self._model_id, attn_implementation="eager"
        ).to(self._device)
        self._model.eval()
        self._processor = WhisperProcessor.from_pretrained(self._model_id)
        self._sr = 16000
        self._policy = BatchPolicy(max_size=batch_size, window_ms=batch_window_ms)

    @property
    def name(self) -> str:
        # Bump when the alignment/post-correction changes (cache key).
        return f"whisper-fa:{self._model_id}:v1"

    @property
    def batch_policy(self) -> BatchPolicy:
        return self._policy

    def call(self, batch: list[Any]) -> list[Any]:
        from batchalign._core.proto import FaInput, FaOutput

        outputs: list[Any] = []
        for item in batch:
            if not isinstance(item, FaInput):
                raise TypeError(
                    f"WhisperFaBackend does not handle input type: {type(item).__name__}"
                )
            outputs.append(self._align_one(item, FaOutput))
        return outputs

    # ----- internals -----------------------------------------------------

    def _dtw_tokens(self, audio_chunk: Any, transcript: str) -> list[tuple[str, float]]:
        """Run the cross-attention DTW aligner on one chunk → `[(token, t_s)]`.

        Port of BA2 `WhisperFAModel.__call__`. Times are relative to the chunk.
        """
        import numpy as np  # type: ignore[import-not-found]
        import torch  # type: ignore[import-not-found]
        from transformers.models.whisper.generation_whisper import (  # type: ignore[import-not-found]
            _dynamic_time_warping as dtw,
            _median_filter as median_filter,
        )

        features = self._processor(
            audio=audio_chunk,
            text=transcript,
            sampling_rate=self._sr,
            return_tensors="pt",
        )
        tokens = features["labels"][0]
        with torch.inference_mode():
            output = self._model(**features.to(self._device), output_attentions=True)

        # decoder cross-attentions: layers × heads × out_tokens × in_frames
        cross_attentions = torch.cat(output.cross_attentions).cpu()
        weights = torch.stack(
            [
                cross_attentions[l][h]
                for l, h in self._model.generation_config.alignment_heads
            ]
        )
        std, mean = torch.std_mean(weights, dim=-2, keepdim=True, unbiased=False)
        weights = (weights - mean) / std
        weights = median_filter(weights, self._model.config.median_filter_width)
        matrix = weights.mean(axis=0)
        # The 0th (<sos>) token's attention smears across the sequence; BA2
        # replaces it with the mean to stop it corrupting the DTW.
        matrix[0] = matrix.mean()

        text_idx, time_idx = dtw(-matrix)
        jumps = np.pad(np.diff(text_idx), (1, 0), constant_values=1).astype(bool)
        jump_times = time_idx[jumps] * _TIME_PRECISION
        return [
            (self._processor.decode(i), float(j)) for i, j in zip(tokens, jump_times)
        ]

    def _align_one(self, item: Any, FaOutput: type) -> Any:
        import numpy as np  # type: ignore[import-not-found]
        import torch  # type: ignore[import-not-found]
        from batchalign._core.proto import AsrSegment, AsrWord

        if not item.utterances:
            return FaOutput(source_id=item.source_id, utterances=[])

        wave = torch.from_numpy(
            np.frombuffer(item.audio.pcm_f32le, dtype=np.float32).copy()
        )
        sr = int(item.audio.sample_rate)

        timings: list[list[Any]] = [[None] * len(u.words) for u in item.utterances]
        windows = [
            (int(getattr(u, "start_ms", 0) or 0), int(getattr(u, "end_ms", 0) or 0))
            for u in item.utterances
        ]

        # BA2-style ~20 s groups of (uidx, widx) over utterances with a window.
        groups: list[list[tuple[int, int]]] = []
        group: list[tuple[int, int]] = []
        seg_start: int | None = None
        for uidx, utt in enumerate(item.utterances):
            w0, w1 = windows[uidx]
            if w1 <= w0:
                continue
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

        for grp in groups:
            if not grp:
                continue
            g_start = windows[grp[0][0]][0]
            g_end = windows[grp[-1][0]][1]
            lo = int(g_start * sr / 1000)
            hi = min(int(g_end * sr / 1000), wave.shape[0])
            if hi <= lo:
                continue
            chunk = wave[lo:hi].cpu().numpy()

            src_words = [item.utterances[u].words[w].text for (u, w) in grp]
            transcript = _strip_word(" ".join(src_words).replace("_", " "))
            if not transcript:
                continue
            try:
                res = self._dtw_tokens(chunk, transcript)
            except Exception:
                continue

            # Char-DP map DTW tokens back to source words (BA2).
            ref_targets = [
                ReferenceTarget(ch, payload=i)
                for i, (u, w) in enumerate(grp)
                for ch in item.utterances[u].words[w].text
            ]
            payload_targets = []
            res_times = []
            for ri, (tok, t) in enumerate(res):
                res_times.append(t)
                for ch in tok:
                    payload_targets.append(PayloadTarget(ch, payload=ri))
            alignments = align(payload_targets, ref_targets, tqdm=False)
            # Reversed so the FIRST timestamp seen for a word wins (BA2).
            alignments.reverse()
            for elem in alignments:
                if isinstance(elem, Match):
                    gi = elem.reference_payload
                    uidx, widx = grp[gi]
                    t = res_times[elem.payload]
                    start = int(round(t * 1000 + g_start))
                    timings[uidx][widx] = (start, start)

        # Post-correct (BA2 whisper_fa.py:183-224): bump each word's end to the
        # next timed word's start, bound by the utterance window; drop impossible.
        aligned: list[Any] = []
        for uidx, utt in enumerate(item.utterances):
            w0, w1 = windows[uidx]
            wt = timings[uidx]
            n = len(utt.words)
            words_out = []
            for widx, word in enumerate(utt.words):
                t = wt[widx]
                if t is not None and widx != n - 1:
                    nxt = widx + 1
                    while nxt < n - 1 and wt[nxt] is None:
                        nxt += 1
                    if wt[nxt] is None:
                        t = (t[0], t[0] + 500)
                    else:
                        t = (t[0], wt[nxt][0])
                elif t is not None:
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


__all__ = ["WhisperFaBackend"]
