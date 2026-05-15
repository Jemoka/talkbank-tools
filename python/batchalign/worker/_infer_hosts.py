"""Bootstrap-owned runtime hosts for worker batch inference.

This module keeps request-time dispatch thin for text tasks that use the
``BatchInferRequest`` dispatch path. The worker bootstrap layer decides
which concrete engines are loaded and registers one batch-infer handler per
task. Request-time code in ``_infer.py`` then routes requests to those
prepared handlers instead of re-deriving engine policy on every call.
"""

from __future__ import annotations

import logging
import threading

from batchalign.worker._types import (
    BatchInferHandler,
    BatchInferRequest,
    BatchInferResponse,
    InferResponse,
    _state,
)

L = logging.getLogger("batchalign.worker")


def unsupported_batch_infer(message: str) -> BatchInferHandler:
    """Build a handler that reports one consistent bootstrap/runtime error."""

    def _handler(req: BatchInferRequest) -> BatchInferResponse:
        """Return the same structured error for every batch item."""
        return BatchInferResponse(
            results=[
                InferResponse(error=message, elapsed_s=0.0)
                for _ in req.items
            ]
        )

    return _handler


def build_morphosyntax_batch_infer_handler() -> BatchInferHandler:
    """Build the morphosyntax batch handler from loaded Stanza runtime state."""
    from pydantic import ValidationError

    from batchalign.inference.morphosyntax import (
        MorphosyntaxBatchItem,
        batch_infer_morphosyntax,
    )
    from batchalign.runtime import FREE_THREADED
    from batchalign.worker._stanza_loading import load_stanza_models

    def _handler(req: BatchInferRequest) -> BatchInferResponse:
        """Run morphosyntax batch inference, loading extra languages on demand."""
        if _state.stanza_pipelines is None:
            return unsupported_batch_infer("No Stanza models loaded")(req)

        lang_set = {req.lang}
        for raw_item in req.items:
            try:
                parsed = MorphosyntaxBatchItem.model_validate(raw_item)
            except ValidationError:
                continue
            if parsed.lang:
                lang_set.add(parsed.lang)

        for lang in sorted(lang_set):
            if lang not in _state.stanza_pipelines:
                try:
                    load_stanza_models(lang)
                except Exception as exc:
                    L.warning("Failed to load Stanza for %s: %s", lang, exc)

        nlp_lock = _state.stanza_nlp_lock
        if nlp_lock is None:
            nlp_lock = threading.Lock()

        return batch_infer_morphosyntax(
            req=req,
            nlp_pipelines=_state.stanza_pipelines,
            contexts=_state.stanza_contexts or {},
            nlp_lock=nlp_lock,
            free_threaded=FREE_THREADED,
            mwt_lexicon=req.mwt,
            progress_callback=_state.active_progress_callback,
        )

    return _handler


def build_utseg_batch_infer_handler() -> BatchInferHandler:
    """Build the utterance-segmentation batch handler from loaded builder state."""
    from batchalign.inference.utseg import batch_infer_utseg

    def _handler(req: BatchInferRequest) -> BatchInferResponse:
        """Run utterance segmentation using the configured Stanza config builder."""
        if _state.utseg_config_builder is None:
            return unsupported_batch_infer("No utseg config builder loaded")(req)
        return batch_infer_utseg(
            req=req,
            build_stanza_config=_state.utseg_config_builder,
            utterance_boundary_model=_state.utterance_boundary_model,
        )

    return _handler


def build_translate_batch_infer_handler() -> BatchInferHandler:
    """Build the translation batch handler from the loaded translation engine."""
    from batchalign.inference.translate import batch_infer_translate

    def _handler(req: BatchInferRequest) -> BatchInferResponse:
        """Run translation using the engine selected during worker bootstrap."""
        if _state.translate_fn is None or _state.translate_backend is None:
            return unsupported_batch_infer("No translation engine loaded")(req)
        return batch_infer_translate(
            req=req,
            translate_fn=_state.translate_fn,
            backend=_state.translate_backend,
        )

    return _handler


__all__ = [
    "BatchInferHandler",
    "build_morphosyntax_batch_infer_handler",
    "build_translate_batch_infer_handler",
    "build_utseg_batch_infer_handler",
    "unsupported_batch_infer",
]
