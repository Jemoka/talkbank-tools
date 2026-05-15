"""Live worker-protocol V2 ASR executor.

**See also:** `../../INTERFACE_MAP.md` section "2. ASR Execution V2" for:
- Rust FFI function: `crates/batchalign-pyo3/src/worker_asr_exec.rs::execute_asr_request_v2()`
- Request/response types: `crates/batchalign-types/src/worker_v2/requests.rs`
- Full Rust/Python responsibility split and input/output contracts.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import TYPE_CHECKING, cast

import numpy as np

from batchalign.inference._domain_types import LanguageCode
from batchalign.worker._types import AsrEngine
from batchalign.worker._types_v2 import (
    ExecuteRequestV2,
    ExecuteResponseV2,
    WhisperChunkResultPayloadV2,
)

if TYPE_CHECKING:
    from collections.abc import Callable

    from batchalign.inference.asr import AsrBatchItem, MonologueAsrResponse
    from batchalign.inference.types import WhisperASRHandle


@dataclass(frozen=True, slots=True)
class AsrExecutionHostV2:
    """Injected ASR execution hooks for the live V2 path."""

    local_whisper_runner: Callable[[np.ndarray, LanguageCode], WhisperChunkResultPayloadV2] | None = None
    hk_tencent_runner: Callable[[AsrBatchItem], MonologueAsrResponse] | None = None
    hk_aliyun_runner: Callable[[AsrBatchItem], MonologueAsrResponse] | None = None
    hk_funaudio_runner: Callable[[AsrBatchItem], MonologueAsrResponse] | None = None


def build_default_asr_execution_host_v2(
    *,
    asr_engine: AsrEngine,
    whisper_model: WhisperASRHandle | None,
) -> AsrExecutionHostV2:
    """Build the live V2 ASR host from already loaded worker state."""

    from batchalign.inference.asr import infer_whisper_prepared_audio

    local_whisper_runner = None
    if whisper_model is not None:

        def _run_local_whisper(
            audio: np.ndarray,
            lang: LanguageCode,
        ) -> WhisperChunkResultPayloadV2:
            return cast(
                WhisperChunkResultPayloadV2,
                infer_whisper_prepared_audio(whisper_model, audio, lang),
            )

        local_whisper_runner = _run_local_whisper

    hk_tencent_runner = None
    hk_aliyun_runner = None
    hk_funaudio_runner = None

    if asr_engine is AsrEngine.TENCENT:
        from batchalign.inference.languages.cantonese._tencent_asr import infer_tencent_asr_v2

        hk_tencent_runner = infer_tencent_asr_v2
    elif asr_engine is AsrEngine.ALIYUN:
        from batchalign.inference.languages.cantonese._aliyun_asr import infer_aliyun_asr_v2

        hk_aliyun_runner = infer_aliyun_asr_v2
    elif asr_engine is AsrEngine.FUNAUDIO:
        from batchalign.inference.languages.cantonese._funaudio_asr import infer_funaudio_asr_v2

        hk_funaudio_runner = infer_funaudio_asr_v2

    return AsrExecutionHostV2(
        local_whisper_runner=local_whisper_runner,
        hk_tencent_runner=hk_tencent_runner,
        hk_aliyun_runner=hk_aliyun_runner,
        hk_funaudio_runner=hk_funaudio_runner,
    )


def execute_asr_request_v2(
    request: ExecuteRequestV2,
    host: AsrExecutionHostV2,
) -> ExecuteResponseV2:
    """Execute one live V2 ASR request through the Rust control plane."""

    import batchalign_core

    return ExecuteResponseV2.model_validate_json(
        batchalign_core.execute_asr_request_v2(
            request,
            host.local_whisper_runner,
            host.hk_tencent_runner,
            host.hk_aliyun_runner,
            host.hk_funaudio_runner,
        )
    )


__all__ = [
    "AsrExecutionHostV2",
    "build_default_asr_execution_host_v2",
    "execute_asr_request_v2",
]
