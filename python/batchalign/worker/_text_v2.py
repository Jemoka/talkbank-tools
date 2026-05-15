"""Live worker-protocol V2 executors for batched text tasks.

This module keeps the Python side of text-task V2 intentionally narrow:

- read one Rust-owned prepared batch artifact
- hand the frozen batch to the already loaded model host
- wrap Rust-normalized per-item model outputs into typed V2 batch results

Rust still owns cross-file batching, cache policy, preprocessing, postprocessing,
and CHAT semantics. Python remains a thin model-host boundary.

**See also:** `../../INTERFACE_MAP.md` section "7. Text Task Result Normalization" for:
- Rust FFI function: `crates/batchalign-pyo3/src/worker_text_results.rs`
- Full Rust/Python responsibility split and input/output contracts.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
import time
from typing import TYPE_CHECKING

from pydantic import BaseModel, Field, ValidationError

from batchalign.errors import BatchalignError

from batchalign.inference.coref import (
    CorefBatchItem,
    batch_infer_coref,
)
from batchalign.inference.morphosyntax import MorphosyntaxBatchItem
from batchalign.inference.translate import TranslateBatchItem
from batchalign.inference.utseg import UtsegBatchItem
from batchalign.worker._artifact_inputs_v2 import (
    ArtifactInputErrorV2,
    load_json_attachment_v2,
)
from batchalign.worker._infer_hosts import (
    build_morphosyntax_batch_infer_handler,
    build_translate_batch_infer_handler,
    build_utseg_batch_infer_handler,
)
from batchalign.worker._types import BatchInferRequest, BatchInferResponse, InferTask
from batchalign.worker._types_v2 import (
    CorefRequestV2,
    CorefResultPayloadV2,
    ExecuteErrorV2,
    ExecuteRequestV2,
    ExecuteResponseV2,
    ExecuteSuccessV2,
    InferenceTaskV2,
    MorphosyntaxRequestV2,
    MorphosyntaxResultPayloadV2,
    ProtocolErrorCodeV2,
    TranslationResultPayloadV2,
    TranslateRequestV2,
    UtsegRequestV2,
    UtsegResultPayloadV2,
)

if TYPE_CHECKING:
    from collections.abc import Callable, Sequence


class MorphosyntaxPreparedBatchV2(BaseModel):
    """Prepared morphosyntax batch payload frozen by Rust."""

    items: list[MorphosyntaxBatchItem]
    mwt: dict[str, list[str]] = Field(default_factory=dict)


class UtsegPreparedBatchV2(BaseModel):
    """Prepared utterance-segmentation batch payload frozen by Rust."""

    items: list[UtsegBatchItem]


class TranslatePreparedBatchV2(BaseModel):
    """Prepared translation batch payload frozen by Rust."""

    items: list[TranslateBatchItem]


class CorefPreparedBatchV2(BaseModel):
    """Prepared coreference batch payload frozen by Rust."""

    items: list[CorefBatchItem]


@dataclass(frozen=True, slots=True)
class TextExecutionHostV2:
    """Injected text-task execution hooks for the live V2 path."""

    morphosyntax_runner: Callable[[BatchInferRequest], BatchInferResponse] | None = None
    utseg_runner: Callable[[BatchInferRequest], BatchInferResponse] | None = None
    translate_runner: Callable[[BatchInferRequest], BatchInferResponse] | None = None
    coref_runner: Callable[[BatchInferRequest], BatchInferResponse] | None = None


def build_default_text_execution_host_v2() -> TextExecutionHostV2:
    """Build the live text-task V2 host from already loaded worker state."""

    return TextExecutionHostV2(
        morphosyntax_runner=build_morphosyntax_batch_infer_handler(),
        utseg_runner=build_utseg_batch_infer_handler(),
        translate_runner=build_translate_batch_infer_handler(),
        coref_runner=batch_infer_coref,
    )


def execute_morphosyntax_request_v2(
    request: ExecuteRequestV2,
    host: TextExecutionHostV2,
) -> ExecuteResponseV2:
    """Execute one batched morphosyntax V2 request."""

    started_at = time.monotonic()

    try:
        morphosyntax_request = _extract_morphosyntax_request(request)
        batch = MorphosyntaxPreparedBatchV2.model_validate(
            load_json_attachment_v2(request.attachments, morphosyntax_request.payload_ref_id)
        )
        _validate_item_count(batch.items, morphosyntax_request.item_count, "morphosyntax")

        # Set up progress emission: the morphosyntax handler reads the
        # progress callback from worker state (thread-local-safe because
        # the sequential stdio loop is single-threaded).
        from batchalign.worker._protocol import write_progress_event
        from batchalign.worker._types import _state

        _last_progress_time = [0.0]

        def _on_progress(completed: int, total: int) -> None:
            now = time.monotonic()
            if now - _last_progress_time[0] < 1.0 and completed < total:
                return
            _last_progress_time[0] = now
            write_progress_event(request.request_id, completed, total)

        _state.active_progress_callback = _on_progress
        try:
            response = _require_runner(host.morphosyntax_runner, "morphosyntax")(
                BatchInferRequest(
                    task=InferTask.MORPHOSYNTAX,
                    lang=morphosyntax_request.lang,
                    items=[item.model_dump(mode="json") for item in batch.items],
                    mwt=batch.mwt,
                    retokenize=morphosyntax_request.retokenize,
                )
            )
        finally:
            _state.active_progress_callback = None

        return ExecuteResponseV2(
            request_id=request.request_id,
            outcome=ExecuteSuccessV2(),
            result=_build_morphosyntax_result(response, morphosyntax_request.item_count),
            elapsed_s=time.monotonic() - started_at,
        )
    except ArtifactInputErrorV2 as error:
        return _error_response(
            request,
            code=_artifact_error_code(error),
            message=str(error),
            started_at=started_at,
        )
    except ValidationError as error:
        return _error_response(
            request,
            code=ProtocolErrorCodeV2.INVALID_PAYLOAD,
            message=str(error),
            started_at=started_at,
        )
    except NotImplementedError as error:
        return _error_response(
            request,
            code=ProtocolErrorCodeV2.MODEL_UNAVAILABLE,
            message=str(error),
            started_at=started_at,
        )
    except ValueError as error:
        return _error_response(
            request,
            code=ProtocolErrorCodeV2.INVALID_PAYLOAD,
            message=str(error),
            started_at=started_at,
        )
    except Exception as error:
        return _error_response(
            request,
            code=ProtocolErrorCodeV2.RUNTIME_FAILURE,
            message=str(error),
            started_at=started_at,
        )


def execute_utseg_request_v2(
    request: ExecuteRequestV2,
    host: TextExecutionHostV2,
) -> ExecuteResponseV2:
    """Execute one batched utterance-segmentation V2 request."""

    started_at = time.monotonic()

    try:
        utseg_request = _extract_utseg_request(request)
        batch = UtsegPreparedBatchV2.model_validate(
            load_json_attachment_v2(request.attachments, utseg_request.payload_ref_id)
        )
        _validate_item_count(batch.items, utseg_request.item_count, "utseg")
        response = _require_runner(host.utseg_runner, "utseg")(
            BatchInferRequest(
                task=InferTask.UTSEG,
                lang=utseg_request.lang,
                items=[item.model_dump(mode="json") for item in batch.items],
                allow_stanza_fallback=utseg_request.allow_stanza_fallback,
            )
        )

        return ExecuteResponseV2(
            request_id=request.request_id,
            outcome=ExecuteSuccessV2(),
            result=_build_utseg_result(response, utseg_request.item_count),
            elapsed_s=time.monotonic() - started_at,
        )
    except ArtifactInputErrorV2 as error:
        return _error_response(
            request,
            code=_artifact_error_code(error),
            message=str(error),
            started_at=started_at,
        )
    except ValidationError as error:
        return _error_response(
            request,
            code=ProtocolErrorCodeV2.INVALID_PAYLOAD,
            message=str(error),
            started_at=started_at,
        )
    except NotImplementedError as error:
        return _error_response(
            request,
            code=ProtocolErrorCodeV2.MODEL_UNAVAILABLE,
            message=str(error),
            started_at=started_at,
        )
    except ValueError as error:
        return _error_response(
            request,
            code=ProtocolErrorCodeV2.INVALID_PAYLOAD,
            message=str(error),
            started_at=started_at,
        )
    except Exception as error:
        return _error_response(
            request,
            code=ProtocolErrorCodeV2.RUNTIME_FAILURE,
            message=str(error),
            started_at=started_at,
        )


def execute_translate_request_v2(
    request: ExecuteRequestV2,
    host: TextExecutionHostV2,
) -> ExecuteResponseV2:
    """Execute one batched translation V2 request."""

    started_at = time.monotonic()

    try:
        translate_request = _extract_translate_request(request)
        batch = TranslatePreparedBatchV2.model_validate(
            load_json_attachment_v2(request.attachments, translate_request.payload_ref_id)
        )
        _validate_item_count(batch.items, translate_request.item_count, "translate")
        response = _require_runner(host.translate_runner, "translate")(
            BatchInferRequest(
                task=InferTask.TRANSLATE,
                lang=translate_request.source_lang,
                items=[item.model_dump(mode="json") for item in batch.items],
            )
        )

        return ExecuteResponseV2(
            request_id=request.request_id,
            outcome=ExecuteSuccessV2(),
            result=_build_translate_result(response, translate_request.item_count),
            elapsed_s=time.monotonic() - started_at,
        )
    except ArtifactInputErrorV2 as error:
        return _error_response(
            request,
            code=_artifact_error_code(error),
            message=str(error),
            started_at=started_at,
        )
    except ValidationError as error:
        return _error_response(
            request,
            code=ProtocolErrorCodeV2.INVALID_PAYLOAD,
            message=str(error),
            started_at=started_at,
        )
    except NotImplementedError as error:
        return _error_response(
            request,
            code=ProtocolErrorCodeV2.MODEL_UNAVAILABLE,
            message=str(error),
            started_at=started_at,
        )
    except ValueError as error:
        return _error_response(
            request,
            code=ProtocolErrorCodeV2.INVALID_PAYLOAD,
            message=str(error),
            started_at=started_at,
        )
    except Exception as error:
        return _error_response(
            request,
            code=ProtocolErrorCodeV2.RUNTIME_FAILURE,
            message=str(error),
            started_at=started_at,
        )


def execute_coref_request_v2(
    request: ExecuteRequestV2,
    host: TextExecutionHostV2,
) -> ExecuteResponseV2:
    """Execute one batched coreference V2 request."""

    started_at = time.monotonic()

    try:
        coref_request = _extract_coref_request(request)
        batch = CorefPreparedBatchV2.model_validate(
            load_json_attachment_v2(request.attachments, coref_request.payload_ref_id)
        )
        _validate_item_count(batch.items, coref_request.item_count, "coref")
        response = _require_runner(host.coref_runner, "coref")(
            BatchInferRequest(
                task=InferTask.COREF,
                lang=coref_request.lang,
                items=[item.model_dump(mode="json") for item in batch.items],
            )
        )

        return ExecuteResponseV2(
            request_id=request.request_id,
            outcome=ExecuteSuccessV2(),
            result=_build_coref_result(response, coref_request.item_count),
            elapsed_s=time.monotonic() - started_at,
        )
    except ArtifactInputErrorV2 as error:
        return _error_response(
            request,
            code=_artifact_error_code(error),
            message=str(error),
            started_at=started_at,
        )
    except ValidationError as error:
        return _error_response(
            request,
            code=ProtocolErrorCodeV2.INVALID_PAYLOAD,
            message=str(error),
            started_at=started_at,
        )
    except NotImplementedError as error:
        return _error_response(
            request,
            code=ProtocolErrorCodeV2.MODEL_UNAVAILABLE,
            message=str(error),
            started_at=started_at,
        )
    except ValueError as error:
        return _error_response(
            request,
            code=ProtocolErrorCodeV2.INVALID_PAYLOAD,
            message=str(error),
            started_at=started_at,
        )
    except Exception as error:
        return _error_response(
            request,
            code=ProtocolErrorCodeV2.RUNTIME_FAILURE,
            message=str(error),
            started_at=started_at,
        )


def _extract_morphosyntax_request(request: ExecuteRequestV2) -> MorphosyntaxRequestV2:
    """Validate that one execute request is a morphosyntax V2 request."""

    if request.task is not InferenceTaskV2.MORPHOSYNTAX:
        raise ValueError(f"expected morphosyntax task, got {request.task!s}")
    if not isinstance(request.payload, MorphosyntaxRequestV2):
        raise ValueError("execute payload did not contain morphosyntax request data")
    return request.payload


def _extract_utseg_request(request: ExecuteRequestV2) -> UtsegRequestV2:
    """Validate that one execute request is a utseg V2 request."""

    if request.task is not InferenceTaskV2.UTSEG:
        raise ValueError(f"expected utseg task, got {request.task!s}")
    if not isinstance(request.payload, UtsegRequestV2):
        raise ValueError("execute payload did not contain utseg request data")
    return request.payload


def _extract_translate_request(request: ExecuteRequestV2) -> TranslateRequestV2:
    """Validate that one execute request is a translate V2 request."""

    if request.task is not InferenceTaskV2.TRANSLATE:
        raise ValueError(f"expected translate task, got {request.task!s}")
    if not isinstance(request.payload, TranslateRequestV2):
        raise ValueError("execute payload did not contain translate request data")
    return request.payload


def _extract_coref_request(request: ExecuteRequestV2) -> CorefRequestV2:
    """Validate that one execute request is a coref V2 request."""

    if request.task is not InferenceTaskV2.COREF:
        raise ValueError(f"expected coref task, got {request.task!s}")
    if not isinstance(request.payload, CorefRequestV2):
        raise ValueError("execute payload did not contain coref request data")
    return request.payload


def _build_morphosyntax_result(
    response: BatchInferResponse,
    item_count: int,
) -> MorphosyntaxResultPayloadV2:
    """Convert one host morphosyntax response into the typed V2 result."""

    try:
        return MorphosyntaxResultPayloadV2.model_validate(
            _normalize_text_task_result_payload("morphosyntax", response, item_count)
        )
    except (ValidationError, ValueError, BatchalignError) as error:
        raise RuntimeError(f"invalid morphosyntax host output: {error}") from error


def _build_utseg_result(
    response: BatchInferResponse,
    item_count: int,
) -> UtsegResultPayloadV2:
    """Convert one host utseg response into the typed V2 result."""

    try:
        return UtsegResultPayloadV2.model_validate(
            _normalize_text_task_result_payload("utseg", response, item_count)
        )
    except (ValidationError, ValueError, BatchalignError) as error:
        raise RuntimeError(f"invalid utseg host output: {error}") from error


def _build_translate_result(
    response: BatchInferResponse,
    item_count: int,
) -> TranslationResultPayloadV2:
    """Convert one host translation response into the typed V2 result."""

    try:
        return TranslationResultPayloadV2.model_validate(
            _normalize_text_task_result_payload("translate", response, item_count)
        )
    except (ValidationError, ValueError, BatchalignError) as error:
        raise RuntimeError(f"invalid translate host output: {error}") from error


def _build_coref_result(
    response: BatchInferResponse,
    item_count: int,
) -> CorefResultPayloadV2:
    """Convert one host coref response into the typed V2 result."""

    try:
        return CorefResultPayloadV2.model_validate(
            _normalize_text_task_result_payload("coref", response, item_count)
        )
    except (ValidationError, ValueError, BatchalignError) as error:
        raise RuntimeError(f"invalid coref host output: {error}") from error


def _validate_item_count(items: Sequence[object], expected_count: int, task: str) -> None:
    """Require that a prepared batch payload length matches the request metadata."""

    actual_count = len(items)
    if actual_count != expected_count:
        raise ValueError(
            f"worker protocol V2 {task} payload had {actual_count} items, "
            f"expected {expected_count}"
        )


def _normalize_text_task_result_payload(
    task: str,
    response: BatchInferResponse,
    item_count: int,
) -> object:
    """Delegate provider-agnostic text-result shaping to the Rust bridge."""

    import batchalign_core

    return json.loads(
        batchalign_core.normalize_text_task_result(
            task,
            response,
            item_count,
        )
    )


def _require_runner(
    runner: Callable[[BatchInferRequest], BatchInferResponse] | None,
    task: str,
) -> Callable[[BatchInferRequest], BatchInferResponse]:
    """Require that one live V2 text-task runner exists."""

    if runner is None:
        raise NotImplementedError(f"no {task} host loaded for worker protocol V2")
    return runner


def _artifact_error_code(error: ArtifactInputErrorV2) -> ProtocolErrorCodeV2:
    """Map prepared-artifact failures onto the stable protocol error codes."""

    if "missing worker protocol V2 attachment" in str(error):
        return ProtocolErrorCodeV2.MISSING_ATTACHMENT
    return ProtocolErrorCodeV2.ATTACHMENT_UNREADABLE


def _error_response(
    request: ExecuteRequestV2,
    *,
    code: ProtocolErrorCodeV2,
    message: str,
    started_at: float,
) -> ExecuteResponseV2:
    """Build one typed text-execute error response."""

    return ExecuteResponseV2(
        request_id=request.request_id,
        outcome=ExecuteErrorV2(code=code, message=message),
        result=None,
        elapsed_s=time.monotonic() - started_at,
    )


__all__ = [
    "TextExecutionHostV2",
    "build_default_text_execution_host_v2",
    "execute_coref_request_v2",
    "execute_morphosyntax_request_v2",
    "execute_translate_request_v2",
    "execute_utseg_request_v2",
]
