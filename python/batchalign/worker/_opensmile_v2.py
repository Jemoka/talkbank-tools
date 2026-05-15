"""Live worker-protocol V2 openSMILE executor.

**See also:** `../../INTERFACE_MAP.md` section "4. Media Analysis V2: OpenSMILE" for:
- Rust FFI function: `crates/batchalign-pyo3/src/worker_media_exec.rs::execute_opensmile_request_v2()`
- Full Rust/Python responsibility split and input/output contracts.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import TYPE_CHECKING

from batchalign.inference.opensmile import (
    OpenSmileResponse,
    infer_opensmile_prepared_audio,
)
from batchalign.worker._types_v2 import (
    ExecuteRequestV2,
    ExecuteResponseV2,
)

if TYPE_CHECKING:
    from collections.abc import Callable

    import numpy as np


@dataclass(frozen=True, slots=True)
class OpenSmileExecutionHostV2:
    """Injected openSMILE execution hooks for the live V2 path."""

    prepared_audio_runner: Callable[[np.ndarray, int, str, str, str], OpenSmileResponse] | None = None


def build_default_opensmile_execution_host_v2() -> OpenSmileExecutionHostV2:
    """Build the live V2 openSMILE host from the existing Python adapter."""

    def _run(
        audio: np.ndarray,
        sample_rate_hz: int,
        feature_set: str,
        feature_level: str,
        audio_label: str,
    ) -> OpenSmileResponse:
        return infer_opensmile_prepared_audio(
            audio,
            sample_rate_hz,
            feature_set=feature_set,
            feature_level=feature_level,
            audio_label=audio_label,
        )

    return OpenSmileExecutionHostV2(prepared_audio_runner=_run)


def execute_opensmile_request_v2(
    request: ExecuteRequestV2,
    host: OpenSmileExecutionHostV2,
) -> ExecuteResponseV2:
    """Execute one live V2 openSMILE request through the Rust control plane."""

    import batchalign_core

    return ExecuteResponseV2.model_validate_json(
        batchalign_core.execute_opensmile_request_v2(
            request,
            host.prepared_audio_runner,
        )
    )


__all__ = [
    "OpenSmileExecutionHostV2",
    "build_default_opensmile_execution_host_v2",
    "execute_opensmile_request_v2",
]
