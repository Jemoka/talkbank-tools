"""Infer and ``batch_infer`` dispatch routers.

This module is the request-time counterpart to the worker bootstrap modules.
Its job is deliberately narrow:

- accept typed inference requests from the stdio protocol layer
- route each task to the correct pure inference helper
- inject only the preloaded runtime state that task actually needs

It must not grow into a pipeline-orchestration layer. CHAT parsing, job
ownership, retries, and workflow policy remain on the Rust side.
"""

from __future__ import annotations

from batchalign.worker._types import (
    BatchInferRequest,
    BatchInferResponse,
    InferRequest,
    InferResponse,
    InferTask,
    _state,
)


def _infer(req: InferRequest) -> InferResponse:
    """Dispatch one inference item through the batched inference router.

    The single-item path is intentionally a thin adapter over `batch_infer` so
    task routing logic stays centralized in one place.
    """
    if _state.test_echo:
        if _state.test_delay_ms > 0:
            import time

            time.sleep(_state.test_delay_ms / 1000.0)
        return InferResponse(result=req.payload, elapsed_s=0.0)

    batch_req = BatchInferRequest(task=req.task, lang=req.lang, items=[req.payload])
    batch_resp = _batch_infer(batch_req)
    return batch_resp.results[0] if batch_resp.results else InferResponse(
        error="Empty batch response", elapsed_s=0.0,
    )


def _batch_infer(req: BatchInferRequest) -> BatchInferResponse:
    """Dispatch one batch request to the task-specific inference adapter."""
    if _state.test_echo:
        if _state.test_delay_ms > 0:
            import time

            time.sleep(_state.test_delay_ms / 1000.0)
        return BatchInferResponse(
            results=[InferResponse(result=item, elapsed_s=0.0) for item in req.items]
        )

    handler = _state.batch_infer_handler(req.task) or _STATIC_BATCH_INFER_DISPATCH.get(
        req.task
    )
    if handler is None:
        return BatchInferResponse(
            results=[
                InferResponse(error=f"Unknown task: {req.task}", elapsed_s=0.0)
                for _ in req.items
            ]
        )
    return handler(req)


def _dispatch_coref(req: BatchInferRequest) -> BatchInferResponse:
    """Coref needs no model wiring — uses direct Stanza import."""
    from batchalign.inference.coref import batch_infer_coref

    return batch_infer_coref(req)

_STATIC_BATCH_INFER_DISPATCH = {
    InferTask.COREF: _dispatch_coref,
}
