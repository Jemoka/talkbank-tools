"""FunASR ASR inference provider for built-in HK/Cantonese engines.

New-style provider: a ``load`` function (called once at worker startup)
and an ``infer`` function (called per batch_infer request).
"""

from __future__ import annotations

import logging
import time

from pydantic import ValidationError

from batchalign.worker._types import (
    BatchInferRequest,
    BatchInferResponse,
    InferResponse,
)
from batchalign.inference.asr import (
    AsrBatchItem,
    AsrElement,
    AsrMonologue,
    MonologueAsrResponse,
)

from batchalign.inference._domain_types import LanguageCode

from ._common import EngineOverrides
from ._funaudio_common import FunAudioRecognizer

L = logging.getLogger("batchalign.hk.funaudio_asr")


# ---------------------------------------------------------------------------
# Module-level state (populated by load_funaudio_asr)
# ---------------------------------------------------------------------------

_recognizer: FunAudioRecognizer | None = None


# ---------------------------------------------------------------------------
# Load
# ---------------------------------------------------------------------------


def load_funaudio_asr(lang: LanguageCode, engine_overrides: EngineOverrides | None) -> None:
    """Initialize the FunAudio ASR recognizer.

    Called once at worker startup.  Stores the recognizer in module-level
    state so that ``infer_funaudio_asr`` can use it for every request.

    Parameters
    ----------
    lang : str
        ISO 639-3 language code (e.g. ``"yue"``).
    engine_overrides : EngineOverrides or None
        Optional dict of overrides applied on top of the defaults
        (``model="FunAudioLLM/SenseVoiceSmall"``, ``device="cpu"``).
        Two keys are recognized:

        * ``"funaudio_model"`` — Hugging Face model id of the FunASR
          checkpoint to load (e.g. a Paraformer variant). The
          downstream :class:`FunAudioRecognizer` branches on whether
          the chosen name contains ``"paraformer"``.
        * ``"funaudio_device"`` — torch device string passed through
          to the recognizer (e.g. ``"cuda"`` or ``"mps"``).

        Other keys in the dict are ignored.
    """
    global _recognizer

    model = "FunAudioLLM/SenseVoiceSmall"
    device = "cpu"
    if engine_overrides:
        if "funaudio_model" in engine_overrides:
            model = str(engine_overrides["funaudio_model"])
        if "funaudio_device" in engine_overrides:
            device = str(engine_overrides["funaudio_device"])

    _recognizer = FunAudioRecognizer(lang=lang, model=model, device=device)
    L.info("FunAudio ASR recognizer loaded: lang=%s, model=%s, device=%s", lang, model, device)


# ---------------------------------------------------------------------------
# Infer
# ---------------------------------------------------------------------------


def infer_funaudio_asr(req: BatchInferRequest) -> BatchInferResponse:
    """Process a batch of ASR inference items using FunASR.

    Each item should be an :class:`AsrBatchItem`.  For each item the
    recognizer transcribes the audio and returns the provider-shaped
    monologue payload directly. Rust owns the shared normalization layer.

    Parameters
    ----------
    req : BatchInferRequest
        Batch of ``AsrBatchItem`` payloads.

    Returns
    -------
    BatchInferResponse
        One :class:`InferResponse` per item, each containing a tagged
        monologue payload on success.
    """
    if _recognizer is None:
        return BatchInferResponse(
            results=[
                InferResponse(error="FunAudio ASR not loaded — call load_funaudio_asr first", elapsed_s=0.0)
                for _ in req.items
            ]
        )

    t0 = time.monotonic()
    n = len(req.items)
    results: list[InferResponse] = []

    for item_idx, raw_item in enumerate(req.items):
        try:
            item = AsrBatchItem.model_validate(raw_item)
        except ValidationError:
            results.append(InferResponse(error="Invalid AsrBatchItem", elapsed_s=0.0))
            continue

        try:
            response = _transcribe_to_monologues(item)
            results.append(
                InferResponse(result=response.model_dump(), elapsed_s=0.0)
            )
        except Exception as e:
            L.warning("FunAudio ASR failed for item %d: %s", item_idx, e, exc_info=True)
            results.append(InferResponse(error=str(e), elapsed_s=0.0))

    elapsed = time.monotonic() - t0
    if results:
        first = results[0]
        results[0] = InferResponse(
            result=first.result, error=first.error, elapsed_s=elapsed
        )

    L.info("batch_infer funaudio_asr: %d items, %.3fs", n, elapsed)
    return BatchInferResponse(results=results)


# ---------------------------------------------------------------------------
# Internal helpers
# ---------------------------------------------------------------------------


def _transcribe_to_monologues(item: AsrBatchItem) -> MonologueAsrResponse:
    """Run FunAudio recognizer and return raw speaker monologues."""
    if _recognizer is None:
        raise RuntimeError("FunAudio recognizer not initialized")

    payload, _timed_words = _recognizer.transcribe(item.audio_path)

    return MonologueAsrResponse(
        lang=item.lang,
        monologues=[
            AsrMonologue(
                speaker=monologue["speaker"],
                elements=[
                    AsrElement(
                        value=element["value"],
                        ts=element["ts"],
                        end_ts=element["end_ts"],
                        type=element["type"],
                    )
                    for element in monologue["elements"]
                    if element["value"].strip()
                ],
            )
            for monologue in payload["monologues"]
        ],
    )


def infer_funaudio_asr_v2(item: AsrBatchItem) -> MonologueAsrResponse:
    """Run one typed FunAudio ASR request for worker protocol V2.

    The live V2 worker path calls the transport directly so Python
    does not reassemble a batch envelope for one request.
    """

    return _transcribe_to_monologues(item)
