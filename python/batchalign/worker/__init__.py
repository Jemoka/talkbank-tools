"""Stateless ML inference worker for Rust server IPC.

This package is the Python data-plane process used by the Rust worker pool.
The Rust side spawns this as a child process and communicates over stdin/stdout.

Protocol:
    Startup (worker -> parent):
        {"ready": true, "pid": 12345, "transport": "stdio"}

    Request (parent -> worker):
        {"op": "infer", "request": <InferRequest>}
        {"op": "batch_infer", "request": <BatchInferRequest>}
        {"op": "execute_v2", "request": <ExecuteRequestV2>}
        {"op": "health"}
        {"op": "capabilities"}
        {"op": "shutdown"}

    Response (worker -> parent):
        {"op": "infer", "response": <InferResponse>}
        {"op": "batch_infer", "response": <BatchInferResponse>}
        {"op": "execute_v2", "response": <ExecuteResponseV2>}
        {"op": "health", "response": <HealthResponse>}
        {"op": "capabilities", "response": <CapabilitiesResponse>}
        {"op": "shutdown"}
        {"op": "error", "error": "..."}

Usage:
    uv run python -m batchalign.worker --task asr --lang eng
    uv run python -m batchalign.worker --test-echo --task morphosyntax
"""

from batchalign.worker._types import (
    BatchInferRequest,
    BatchInferResponse,
    CapabilitiesResponse,
    HealthResponse,
    InferTask,
    InferRequest,
    InferResponse,
)


def main() -> None:
    """Lazily resolve the worker CLI entry point.

    The worker package re-exports `main()` for convenience, but importing the
    package for its wire types should not eagerly import the stdio protocol
    implementation.
    """
    from batchalign.worker._main import main as worker_main

    worker_main()

__all__ = [
    "BatchInferRequest",
    "BatchInferResponse",
    "CapabilitiesResponse",
    "HealthResponse",
    "InferTask",
    "InferRequest",
    "InferResponse",
    "main",
]
