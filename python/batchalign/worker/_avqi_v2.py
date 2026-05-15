"""Live worker-protocol V2 AVQI executor.

**See also:** `../../INTERFACE_MAP.md` section "5. Media Analysis V2: AVQI" for:
- Rust FFI function: `crates/batchalign-pyo3/src/worker_media_exec.rs::execute_avqi_request_v2()`
- Full Rust/Python responsibility split and input/output contracts.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import TYPE_CHECKING

from batchalign.inference.avqi import AvqiResponse, infer_avqi_prepared_audio
from batchalign.worker._types_v2 import (
    ExecuteRequestV2,
    ExecuteResponseV2,
)

if TYPE_CHECKING:
    from collections.abc import Callable

    import numpy as np


@dataclass(frozen=True, slots=True)
class AvqiExecutionHostV2:
    """Injected AVQI execution hooks for the live V2 path."""

    prepared_audio_runner: (
        Callable[[np.ndarray, int, np.ndarray, int, str, str], AvqiResponse] | None
    ) = None


def build_default_avqi_execution_host_v2() -> AvqiExecutionHostV2:
    """Build the live V2 AVQI host from the existing Python adapter."""

    def _run(
        cs_audio: np.ndarray,
        cs_sample_rate_hz: int,
        sv_audio: np.ndarray,
        sv_sample_rate_hz: int,
        cs_label: str,
        sv_label: str,
    ) -> AvqiResponse:
        return infer_avqi_prepared_audio(
            cs_audio,
            cs_sample_rate_hz,
            sv_audio,
            sv_sample_rate_hz,
            cs_label=cs_label,
            sv_label=sv_label,
        )

    return AvqiExecutionHostV2(prepared_audio_runner=_run)


def execute_avqi_request_v2(
    request: ExecuteRequestV2,
    host: AvqiExecutionHostV2,
) -> ExecuteResponseV2:
    """Execute one live V2 AVQI request through the Rust control plane."""

    import batchalign_core

    return ExecuteResponseV2.model_validate_json(
        batchalign_core.execute_avqi_request_v2(
            request,
            host.prepared_audio_runner,
        )
    )


__all__ = [
    "AvqiExecutionHostV2",
    "build_default_avqi_execution_host_v2",
    "execute_avqi_request_v2",
]
