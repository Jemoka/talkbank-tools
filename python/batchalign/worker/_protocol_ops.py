"""Thin Python wrapper over Rust-owned worker stdio op dispatch."""

from __future__ import annotations

from dataclasses import dataclass
from typing import cast

import batchalign_core
from pydantic import ValidationError

from batchalign.worker._execute_v2 import execute_request_v2
from batchalign.worker._handlers import _capabilities, _ensure_task, _health
from batchalign.worker._infer import _batch_infer, _infer
from batchalign.worker._types import (
    BatchInferRequest,
    InferRequest,
    WorkerJSONValue,
)
from batchalign.worker._types_v2 import ExecuteRequestV2


@dataclass(frozen=True, slots=True)
class ProtocolDispatchResult:
    """One decoded protocol result ready for JSON-line emission."""

    payload: dict[str, WorkerJSONValue]
    should_shutdown: bool = False


def dispatch_protocol_message(message: object) -> ProtocolDispatchResult:
    """Decode one worker IPC message into a response envelope."""
    payload, should_shutdown = batchalign_core.dispatch_protocol_message(
        message,
        health_fn=_health,
        capabilities_fn=_capabilities,
        infer_fn=_infer,
        batch_infer_fn=_batch_infer,
        execute_v2_fn=execute_request_v2,
        ensure_task_fn=_ensure_task,
        infer_request_model=InferRequest,
        batch_infer_request_model=BatchInferRequest,
        execute_v2_request_model=ExecuteRequestV2,
        validation_error_type=ValidationError,
    )
    return ProtocolDispatchResult(
        payload=cast(dict[str, WorkerJSONValue], payload),
        should_shutdown=should_shutdown,
    )


__all__ = [
    "ProtocolDispatchResult",
    "dispatch_protocol_message",
]
