"""Tencent Cloud ASR provider for built-in HK/Cantonese engines.

New-style provider: a pair of module-level functions (load + infer)
instead of an engine class.  The heavy ``TencentRecognizer`` lives in
``tencent_api.py`` and is kept as-is.
"""

from __future__ import annotations

import configparser
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
from ._tencent_api import TencentRecognizer

L = logging.getLogger("batchalign.hk.tencent")

# ---------------------------------------------------------------------------
# Module-level state
# ---------------------------------------------------------------------------

_recognizer: TencentRecognizer | None = None
_lang: LanguageCode = "eng"


# ---------------------------------------------------------------------------
# Provider interface
# ---------------------------------------------------------------------------


def load_tencent_asr(
    lang: LanguageCode,
    engine_overrides: EngineOverrides | None,
    *,
    config: configparser.ConfigParser | None = None,
) -> None:
    """Create a ``TencentRecognizer`` and store it in module-level state.

    Called once at worker startup when ``--engine-overrides`` selects the
    ``tencent`` ASR provider.
    """
    global _recognizer, _lang
    _lang = lang
    _recognizer = TencentRecognizer(lang=lang, config=config)
    L.info("Tencent ASR provider loaded for lang=%s", lang)


def infer_tencent_asr(req: BatchInferRequest) -> BatchInferResponse:
    """Process a batch of ASR items via the Tencent recognizer.

    Each item is an ``AsrBatchItem`` (audio_path, lang, num_speakers).
    For each item we call the recognizer to get speaker monologues and
    return that provider-shaped payload directly. Rust owns the shared
    token/timing normalization layer after worker deserialization.
    """
    if _recognizer is None:
        return BatchInferResponse(
            results=[
                InferResponse(error="Tencent ASR provider not loaded", elapsed_s=0.0)
                for _ in req.items
            ],
        )

    t0 = time.monotonic()
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
                InferResponse(result=response.model_dump(), elapsed_s=0.0),
            )
        except Exception as exc:
            L.warning(
                "Tencent ASR failed for item %d: %s", item_idx, exc, exc_info=True,
            )
            results.append(InferResponse(error=str(exc), elapsed_s=0.0))

    elapsed = time.monotonic() - t0

    # Stamp total elapsed time on the first result.
    if results:
        first = results[0]
        results[0] = InferResponse(
            result=first.result, error=first.error, elapsed_s=elapsed,
        )

    L.info("batch_infer tencent_asr: %d items, %.3fs", len(req.items), elapsed)
    return BatchInferResponse(results=results)


# ---------------------------------------------------------------------------
# Internal helpers
# ---------------------------------------------------------------------------


def _transcribe_to_monologues(item: AsrBatchItem) -> MonologueAsrResponse:
    """Run Tencent recognition and return raw speaker monologues."""
    if _recognizer is None:
        raise RuntimeError("Tencent recognizer not initialized")

    details = _recognizer.transcribe(item.audio_path, num_speakers=item.num_speakers)
    payload = _recognizer.monologues(details)

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


def infer_tencent_asr_v2(item: AsrBatchItem) -> MonologueAsrResponse:
    """Run one typed Tencent ASR request for worker protocol V2.

    The V2 worker path should not rebuild a fake ``BatchInferRequest`` just to
    reach the loaded recognizer. This helper keeps the provider boundary thin:
    one typed ASR item in, one raw monologue payload out.
    """

    return _transcribe_to_monologues(item)
