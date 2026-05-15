"""Forced alignment inference: audio+text -> timings.

Pure inference — no CHAT, no caching, no pipeline.
"""

from __future__ import annotations

import logging
import threading
import time
from typing import TYPE_CHECKING

import numpy as np
import torch
from pydantic import BaseModel, ValidationError, model_validator

from batchalign.inference._domain_types import (
    AudioPath,
    ConfidenceScore,
    SampleRate,
    TimestampMs,
)

if TYPE_CHECKING:
    from batchalign.inference.types import (
        Wave2VecFAHandle,
        WhisperFAHandle,
    )
    from batchalign.inference.audio import ASRAudioFile

from batchalign.providers import (
    BatchInferRequest,
    BatchInferResponse,
    InferResponse,
)

L = logging.getLogger("batchalign.worker")


# ---------------------------------------------------------------------------
# Pydantic models (FA payload/response)
# ---------------------------------------------------------------------------


class FaInferItem(BaseModel):
    """A single FA inference request item."""

    words: list[str]
    word_ids: list[str]
    word_utterance_indices: list[int]
    word_utterance_word_indices: list[int]
    audio_path: AudioPath
    audio_start_ms: TimestampMs
    audio_end_ms: TimestampMs
    pauses: bool = False

    @model_validator(mode="after")
    def validate_parallel_arrays(self) -> FaInferItem:
        expected = len(self.words)
        if len(self.word_ids) != expected:
            raise ValueError(
                f"word_ids length mismatch: expected {expected}, got {len(self.word_ids)}"
            )
        if len(self.word_utterance_indices) != expected:
            raise ValueError(
                f"word_utterance_indices length mismatch: expected {expected}, "
                f"got {len(self.word_utterance_indices)}"
            )
        if len(self.word_utterance_word_indices) != expected:
            raise ValueError(
                f"word_utterance_word_indices length mismatch: expected {expected}, "
                f"got {len(self.word_utterance_word_indices)}"
            )
        return self


class FaRawToken(BaseModel):
    """A single raw token with timestamp from Whisper."""

    text: str
    time_s: float


class WhisperFaResponse(BaseModel):
    """Whisper FA output: raw tokens with timestamps."""

    tokens: list[FaRawToken]


class FaIndexedTiming(BaseModel):
    """A word-level timing result."""

    start_ms: TimestampMs
    end_ms: TimestampMs
    confidence: ConfidenceScore | None = None


class Wave2VecIndexedResponse(BaseModel):
    """Wave2Vec FA output: indexed timings aligned to input words."""

    indexed_timings: list[FaIndexedTiming | None]


# ---------------------------------------------------------------------------
# Whisper FA load/infer
# ---------------------------------------------------------------------------


def load_whisper_fa(
    model: str = "openai/whisper-large-v2",
    target_sample_rate: SampleRate = 16000,
    *,
    device_policy=None,
) -> WhisperFAHandle:
    """Load a Whisper FA model. Returns a typed handle."""
    import torch
    from transformers import WhisperForConditionalGeneration, WhisperProcessor

    from batchalign.inference.audio import bind_whisper_token_timestamp_extractor
    from batchalign.inference.types import WhisperFAHandle
    from batchalign.device import resolve_inference_device
    from batchalign.worker._progress import (
        HF_ARTIFACTS_WHISPER,
        emit_hf_download_if_missing,
    )

    device = resolve_inference_device(device_policy)

    if device.type == "cuda":
        torch_dtype = torch.float16
    else:
        torch_dtype = torch.float32

    # Multi-GB Whisper FA model download — make the wait visible. Probe
    # the full Whisper artifact set so a partial-cache state (e.g.,
    # tokenizer.json evicted while weights remain) still triggers a
    # download notification.
    emit_hf_download_if_missing(
        model, kind="forced alignment", artifacts=HF_ARTIFACTS_WHISPER
    )

    whisper_model = WhisperForConditionalGeneration.from_pretrained(
        model, attn_implementation="eager", torch_dtype=torch_dtype
    )
    bind_whisper_token_timestamp_extractor(whisper_model)
    whisper_model.to(device)
    whisper_model.eval()
    processor = WhisperProcessor.from_pretrained(model)

    return WhisperFAHandle(
        model=whisper_model,
        processor=processor,
        sample_rate=target_sample_rate,
    )


def infer_whisper_fa(
    handle: WhisperFAHandle,
    audio_chunk: torch.Tensor,
    text: str,
    pauses: bool = False,
) -> list[tuple[str, float]]:
    """Run Whisper forced alignment. Returns [(token_text, timestamp_sec), ...]."""
    import torch
    from transformers.models.whisper.generation_whisper import (
        _dynamic_time_warping as dtw,
        _median_filter as median_filter,
    )

    device = next(handle.model.parameters()).device

    words_list = list(text) if pauses else text
    features = handle.processor(
        audio=audio_chunk, text=(" ".join(words_list) if pauses else text),
        sampling_rate=handle.sample_rate, return_tensors="pt",
    )
    tokens = features["labels"][0]

    with torch.inference_mode():
        output = handle.model(**features.to(device), output_attentions=True)

    cross_attentions = torch.cat(output.cross_attentions).cpu()
    weights = torch.stack([
        cross_attentions[l][h]
        for l, h in handle.model.generation_config.alignment_heads
    ])

    std, mean = torch.std_mean(weights, dim=-2, keepdim=True, unbiased=False)
    weights = (weights - mean) / std
    weights = median_filter(weights, handle.model.config.median_filter_width)
    matrix = weights.mean(dim=0)
    matrix[0] = matrix.mean()

    text_idx, time_idx = dtw(-matrix)
    jumps = np.pad(np.diff(text_idx), (1, 0), constant_values=1).astype(bool)
    jump_times = time_idx[jumps] * 0.02

    return [(handle.processor.decode(i), j) for i, j in zip(tokens, jump_times)]


# ---------------------------------------------------------------------------
# Wave2Vec FA load/infer (replaces Wave2VecFAModel class)
# ---------------------------------------------------------------------------


def load_wave2vec_fa(
    target_sample_rate: SampleRate = 16000,
    *,
    device_policy=None,
) -> Wave2VecFAHandle:
    """Load a Wave2Vec FA model. Returns a typed handle."""
    import torch
    import torchaudio

    from batchalign.inference.types import Wave2VecFAHandle
    from batchalign.device import resolve_inference_device

    from batchalign.worker._progress import emit_download_event

    bundle = torchaudio.pipelines.MMS_FA
    device = resolve_inference_device(device_policy)

    # ``MMS_FA.get_model()`` downloads to torchaudio's hub cache on first use
    # (~1.2 GB). torchaudio prints its own progress to stderr; surface a
    # parallel event on the BA3 protocol channel so every UI sees the wait.
    # Best-effort cache check: torchaudio doesn't expose a clean API for
    # this, so we always emit. False positives (cached, but we still notify)
    # are a much smaller UX cost than silent multi-minute waits.
    emit_download_event(
        stage="downloading_torchaudio_mms_fa",
        user_message=(
            "Downloading Wave2Vec MMS_FA bundle for forced alignment "
            "(one-time, ~1.2 GB; future runs will use the local cache)…"
        ),
    )
    model = bundle.get_model()
    model = model.to(device)
    return Wave2VecFAHandle(model=model, sample_rate=target_sample_rate)


def infer_wave2vec_fa(
    handle: Wave2VecFAHandle,
    audio_chunk: torch.Tensor,
    words: list[str],
) -> list[tuple[str, tuple[int, int]]]:
    """Run Wave2Vec forced alignment. Returns [(word, (start_ms, end_ms)), ...]."""
    import torch
    import torchaudio.functional as AF
    from torchaudio.pipelines import MMS_FA as bundle

    def _build_target_tokens(
        source_words: list[str],
        dictionary: dict[str, int],
    ) -> tuple[torch.Tensor, list[int]]:
        # MMS_FA uses CTC blank index 0 internally, and at least '-' maps there in
        # the live dictionary. Strip blank-mapped chars at the engine boundary
        # instead of changing the shared word model. If a word would become empty,
        # fall back to the wildcard token so word-slot accounting still works.
        wildcard = dictionary["*"]
        blank_index = 0
        transcript_tokens: list[int] = []
        word_lengths: list[int] = []

        for word in source_words:
            word_tokens: list[int] = []
            for char in word.lower():
                token = dictionary.get(char, wildcard)
                if token == blank_index:
                    continue
                word_tokens.append(token)
            if not word_tokens:
                word_tokens = [wildcard]
            transcript_tokens.extend(word_tokens)
            word_lengths.append(len(word_tokens))

        return torch.tensor(transcript_tokens, dtype=torch.int64), word_lengths

    device = next(handle.model.parameters()).device

    audio = audio_chunk.to(device)
    emission, _ = handle.model(audio.unsqueeze(0))
    emission = emission.cpu().detach()

    dictionary = bundle.get_dict()
    transcript, word_lengths = _build_target_tokens(words, dictionary)

    path, scores = AF.forced_align(emission, transcript.unsqueeze(0))
    alignments, scores = path[0], scores[0]
    scores = scores.exp()
    merged_path = AF.merge_tokens(alignments, scores)

    def unflatten(list_: list[torch.Tensor], lengths: list[int]) -> list[list[torch.Tensor]]:
        i = 0
        ret = []
        for length in lengths:
            ret.append(list_[i : i + length])
            i += length
        return ret

    word_spans = unflatten(merged_path, word_lengths)
    ratio = audio.size(0) / emission.size(1)
    result: list[tuple[str, tuple[int, int]]] = [
        (word, (int(((spans[0].start * ratio) / handle.sample_rate) * 1000),
                int(((spans[-1].end * ratio) / handle.sample_rate) * 1000)))
        for word, spans in zip(words, word_spans)
    ]
    return result


# ---------------------------------------------------------------------------
# Inference function
# ---------------------------------------------------------------------------


def batch_infer_fa(
    req: BatchInferRequest,
    whisper_model: WhisperFAHandle | None,
    wave2vec_model: Wave2VecFAHandle | None,
) -> BatchInferResponse:
    """Batch FA inference: (audio_chunk, words) -> raw timings.

    Whisper always returns raw tokens (WhisperFaResponse); Rust handles
    token-to-word alignment via deterministic stitching + DP fallback.
    Wave2Vec returns indexed word-level timings (Wave2VecIndexedResponse).
    """
    from batchalign.inference.audio import load_audio_file

    is_whisper = whisper_model is not None

    t0 = time.monotonic()

    audio_cache: dict[str, ASRAudioFile] = {}
    lock = threading.Lock()

    n = len(req.items)
    results: list[InferResponse] = []

    for item_idx, raw_item in enumerate(req.items):
        try:
            item = FaInferItem.model_validate(raw_item)
        except ValidationError:
            results.append(InferResponse(error="Invalid FaInferItem", elapsed_s=0.0))
            continue

        if not item.words:
            if is_whisper:
                results.append(
                    InferResponse(
                        result=WhisperFaResponse(tokens=[]).model_dump(),
                        elapsed_s=0.0,
                    )
                )
            else:
                results.append(
                    InferResponse(
                        result=Wave2VecIndexedResponse(indexed_timings=[]).model_dump(),
                        elapsed_s=0.0,
                    )
                )
            continue

        try:
            if item.audio_path not in audio_cache:
                audio_cache[item.audio_path] = load_audio_file(item.audio_path)
            audio_obj = audio_cache[item.audio_path]
            audio_chunk = audio_obj.chunk(item.audio_start_ms, item.audio_end_ms)

            if is_whisper:
                assert whisper_model is not None
                detokenized = " ".join(item.words)
                detokenized = detokenized.replace("_", " ").strip()
                with lock:
                    whisper_results = infer_whisper_fa(
                        whisper_model, audio_chunk, detokenized, pauses=item.pauses
                    )
                # Always return raw tokens — Rust handles alignment.
                response_data = WhisperFaResponse(
                    tokens=[
                        FaRawToken(text=text, time_s=t)
                        for text, t in whisper_results
                    ]
                ).model_dump()
            else:
                assert wave2vec_model is not None
                with lock:
                    wave2vec_results = infer_wave2vec_fa(
                        wave2vec_model, audio_chunk, item.words
                    )
                indexed_timings_list: list[FaIndexedTiming | None] = [None] * len(
                    item.words
                )
                for i, (_, (start, end)) in enumerate(
                    wave2vec_results[: len(item.words)]
                ):
                    indexed_timings_list[i] = FaIndexedTiming(
                        start_ms=start, end_ms=end
                    )
                response_data = Wave2VecIndexedResponse(
                    indexed_timings=indexed_timings_list
                ).model_dump()

            results.append(InferResponse(result=response_data, elapsed_s=0.0))

        except Exception as e:
            L.warning(
                "FA infer failed for item %d: %s", item_idx, e, exc_info=True
            )
            if is_whisper:
                results.append(
                    InferResponse(
                        result=WhisperFaResponse(tokens=[]).model_dump(),
                        elapsed_s=0.0,
                    )
                )
            else:
                results.append(
                    InferResponse(
                        result=Wave2VecIndexedResponse(
                            indexed_timings=[]
                        ).model_dump(),
                        elapsed_s=0.0,
                    )
                )

    elapsed = time.monotonic() - t0
    if results:
        first = results[0]
        results[0] = InferResponse(
            result=first.result, error=first.error, elapsed_s=elapsed
        )

    L.info(
        "batch_infer fa: %d items (%s), %.3fs",
        n,
        "whisper" if is_whisper else "wave2vec",
        elapsed,
    )
    return BatchInferResponse(results=results)
