"""Translation inference: text -> translated text.

Pure inference — no CHAT, no caching, no pipeline.
"""

from __future__ import annotations

import logging
import time
from collections.abc import Callable

from pydantic import BaseModel, ValidationError

from batchalign.inference._domain_types import TranslationBackend
from batchalign.providers import (
    BatchInferRequest,
    BatchInferResponse,
    InferResponse,
)

L = logging.getLogger("batchalign.worker")


class TranslateBatchItem(BaseModel):
    """A single item in the batch translate payload from Rust."""

    text: str



def batch_infer_translate(
    req: BatchInferRequest,
    translate_fn: Callable[[str, str], str],
    backend: TranslationBackend,
) -> BatchInferResponse:
    """Batch translation inference: text -> translation.

    Parameters
    ----------
    req : BatchInferRequest
        Batch of TranslateBatchItem payloads.
    translate_fn : callable
        Function ``(text, src_lang) -> str`` that performs translation.
    backend : TranslationBackend
        Which translation engine is active; Google requires rate limiting.
    """
    _translate = translate_fn

    t0 = time.monotonic()
    src_lang = req.lang if req.lang else "eng"

    results: list[InferResponse] = []
    for raw_item in req.items:
        try:
            item = TranslateBatchItem.model_validate(raw_item)
        except ValidationError:
            results.append(InferResponse(error="Invalid batch item", elapsed_s=0.0))
            continue

        if not item.text.strip():
            results.append(
                InferResponse(
                    result={"raw_translation": ""},
                    elapsed_s=0.0,
                )
            )
            continue

        try:
            # Text arrives pre-processed from Rust (Chinese space removal etc.).
            # Return raw translation output — Rust handles post-processing.
            translated = _translate(item.text, src_lang)

            results.append(
                InferResponse(
                    result={"raw_translation": translated},
                    elapsed_s=0.0,
                )
            )
        except Exception as e:
            L.warning("Translation failed for item: %s", e, exc_info=True)
            results.append(
                InferResponse(error=f"Translation failed: {e}", elapsed_s=0.0)
            )

        if backend == TranslationBackend.GOOGLE:
            time.sleep(1.5)

    elapsed = time.monotonic() - t0
    if results:
        first = results[0]
        results[0] = InferResponse(
            result=first.result, error=first.error, elapsed_s=elapsed
        )

    L.info("batch_infer translate: %d items, %.3fs", len(req.items), elapsed)
    return BatchInferResponse(results=results)
