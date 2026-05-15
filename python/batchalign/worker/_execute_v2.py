"""Live worker-protocol V2 execution boundary.

This module is the narrow typed entrypoint for live V2 execution over the
existing stdio worker process. Its job is intentionally small:

- build model-host adapters from already loaded worker state
- route one typed V2 execute request to the correct task executor
- return one typed V2 execute response

The production worker loop should not assemble FA hosts or branch on loaded
model state inline. Keeping that wiring here makes the dispatch from
``BatchInferRequest`` payloads to typed V2 execute requests explicit and testable.

**See also:** `../../INTERFACE_MAP.md` for the unified Python/Rust interface
reference, including Rust FFI function signatures, shared schema definitions,
and full Python caller locations.
"""

from __future__ import annotations

from dataclasses import dataclass, field

from batchalign.worker._asr_v2 import (
    AsrExecutionHostV2,
    build_default_asr_execution_host_v2,
    execute_asr_request_v2,
)
from batchalign.worker._avqi_v2 import (
    AvqiExecutionHostV2,
    build_default_avqi_execution_host_v2,
    execute_avqi_request_v2,
)
from batchalign.worker._fa_v2 import (
    ForcedAlignmentExecutionHostV2,
    build_default_fa_execution_host_v2,
    execute_forced_alignment_request_v2,
)
from batchalign.worker._opensmile_v2 import (
    OpenSmileExecutionHostV2,
    build_default_opensmile_execution_host_v2,
    execute_opensmile_request_v2,
)
from batchalign.worker._speaker_v2 import (
    SpeakerExecutionHostV2,
    build_default_speaker_execution_host_v2,
    execute_speaker_request_v2,
)
from batchalign.worker._text_v2 import (
    TextExecutionHostV2,
    build_default_text_execution_host_v2,
    execute_coref_request_v2,
    execute_morphosyntax_request_v2,
    execute_translate_request_v2,
    execute_utseg_request_v2,
)
from batchalign.worker._types import _state
from batchalign.worker._types_v2 import (
    ExecuteErrorV2,
    ExecuteRequestV2,
    ExecuteResponseV2,
    InferenceTaskV2,
    ProtocolErrorCodeV2,
)


@dataclass(frozen=True, slots=True)
class WorkerExecutionHostV2:
    """Live V2 execution hosts built from already loaded worker state.

    The host groups the task-specific executors that the Python worker can run
    directly. Tests inject typed fake hosts here instead of replacing module
    globals.
    """

    asr: AsrExecutionHostV2 = field(default_factory=AsrExecutionHostV2)
    forced_alignment: ForcedAlignmentExecutionHostV2 = field(
        default_factory=ForcedAlignmentExecutionHostV2
    )
    speaker: SpeakerExecutionHostV2 = field(default_factory=SpeakerExecutionHostV2)
    opensmile: OpenSmileExecutionHostV2 = field(default_factory=OpenSmileExecutionHostV2)
    avqi: AvqiExecutionHostV2 = field(default_factory=AvqiExecutionHostV2)
    text: TextExecutionHostV2 = field(default_factory=TextExecutionHostV2)


def build_default_execution_host_v2() -> WorkerExecutionHostV2:
    """Build the live V2 execution hosts from loaded worker models."""

    return WorkerExecutionHostV2(
        asr=build_default_asr_execution_host_v2(
            asr_engine=_state.asr_engine,
            whisper_model=_state.whisper_asr_model,
        ),
        forced_alignment=build_default_fa_execution_host_v2(
            whisper_model=_state.whisper_fa_model,
            wave2vec_model=_state.wave2vec_fa_model,
        ),
        speaker=build_default_speaker_execution_host_v2(
            _state.bootstrap.device_policy if _state.bootstrap is not None else None
        ),
        opensmile=build_default_opensmile_execution_host_v2(),
        avqi=build_default_avqi_execution_host_v2(),
        text=build_default_text_execution_host_v2(),
    )


def execute_request_v2(
    request: ExecuteRequestV2,
    *,
    host: WorkerExecutionHostV2 | None = None,
) -> ExecuteResponseV2:
    """Execute one typed V2 worker request against the loaded runtime."""

    invalid_request_response = _validate_request_boundary(request)
    if invalid_request_response is not None:
        return invalid_request_response

    # Test-echo mode: return a successful echo response without model dispatch.
    # This enables integration tests for the concurrent dispatch path
    # (SharedGpuWorker) without loading real ML models.
    if _state.test_echo:
        import time

        from batchalign.worker._types_v2 import ExecuteSuccessV2

        if _state.test_delay_ms > 0:
            time.sleep(_state.test_delay_ms / 1000.0)

        return ExecuteResponseV2(
            request_id=request.request_id,
            outcome=ExecuteSuccessV2(),
            result=None,
            elapsed_s=0.001,
        )

    execution_host = host or build_default_execution_host_v2()

    match request.task:
        case InferenceTaskV2.MORPHOSYNTAX:
            return execute_morphosyntax_request_v2(request, execution_host.text)
        case InferenceTaskV2.UTSEG:
            return execute_utseg_request_v2(request, execution_host.text)
        case InferenceTaskV2.TRANSLATE:
            return execute_translate_request_v2(request, execution_host.text)
        case InferenceTaskV2.COREF:
            return execute_coref_request_v2(request, execution_host.text)
        case InferenceTaskV2.ASR:
            return execute_asr_request_v2(request, execution_host.asr)
        case InferenceTaskV2.FORCED_ALIGNMENT:
            return execute_forced_alignment_request_v2(
                request,
                execution_host.forced_alignment,
            )
        case InferenceTaskV2.SPEAKER:
            return execute_speaker_request_v2(request, execution_host.speaker)
        case InferenceTaskV2.OPENSMILE:
            return execute_opensmile_request_v2(request, execution_host.opensmile)
        case InferenceTaskV2.AVQI:
            return execute_avqi_request_v2(request, execution_host.avqi)
        case _:
            return _unsupported_task_response(request)


def _unsupported_task_response(request: ExecuteRequestV2) -> ExecuteResponseV2:
    """Return a typed error for V2 tasks that are not live yet."""

    return ExecuteResponseV2(
        request_id=request.request_id,
        outcome=ExecuteErrorV2(
            code=ProtocolErrorCodeV2.MODEL_UNAVAILABLE,
            message=(
                f"worker protocol V2 task {request.task.value} is not wired into "
                "the live worker yet"
            ),
        ),
        result=None,
        elapsed_s=0.0,
    )


def _validate_request_boundary(request: ExecuteRequestV2) -> ExecuteResponseV2 | None:
    """Reject mismatched top-level task/payload combinations before dispatch."""

    payload_kind = getattr(request.payload, "kind", None)
    if payload_kind is None:
        return _invalid_payload_response(
            request,
            "execute payload did not include a task kind discriminator",
        )
    if request.task.value != payload_kind:
        return _invalid_payload_response(
            request,
            f"execute payload kind {payload_kind} does not match task {request.task.value}",
        )
    return None


def _invalid_payload_response(
    request: ExecuteRequestV2,
    message: str,
) -> ExecuteResponseV2:
    """Return one typed invalid-payload protocol response."""

    return ExecuteResponseV2(
        request_id=request.request_id,
        outcome=ExecuteErrorV2(
            code=ProtocolErrorCodeV2.INVALID_PAYLOAD,
            message=message,
        ),
        result=None,
        elapsed_s=0.0,
    )


__all__ = [
    "WorkerExecutionHostV2",
    "build_default_execution_host_v2",
    "execute_request_v2",
]
