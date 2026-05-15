"""Live worker-protocol V2 speaker diarization executor.

**See also:** `../../INTERFACE_MAP.md` section "6. Media Analysis V2: Speaker Diarization" for:
- Rust FFI function: `crates/batchalign-pyo3/src/worker_media_exec.rs::execute_speaker_request_v2()`
- Full Rust/Python responsibility split and input/output contracts.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import TYPE_CHECKING

from batchalign.device import DevicePolicy
from batchalign.inference.speaker import (
    SpeakerResponse,
    infer_speaker_prepared_audio,
)
from batchalign.worker._types_v2 import (
    ExecuteRequestV2,
    ExecuteResponseV2,
)

if TYPE_CHECKING:
    from collections.abc import Callable

    import numpy as np


@dataclass(frozen=True, slots=True)
class SpeakerExecutionHostV2:
    """Injected speaker execution hooks for the live V2 path."""

    pyannote_prepared_audio_runner: Callable[[np.ndarray, int, int], SpeakerResponse] | None = None
    nemo_prepared_audio_runner: Callable[[np.ndarray, int, int], SpeakerResponse] | None = None


def build_default_speaker_execution_host_v2(
    device_policy: DevicePolicy | None = None,
) -> SpeakerExecutionHostV2:
    """Build the live V2 speaker host from the existing Python adapters."""

    def _run_prepared(
        audio: np.ndarray,
        sample_rate_hz: int,
        num_speakers: int,
        engine: str,
    ) -> SpeakerResponse:
        return infer_speaker_prepared_audio(
            audio,
            sample_rate_hz,
            num_speakers=num_speakers,
            engine=engine,
            device_policy=device_policy,
        )

    return SpeakerExecutionHostV2(
        pyannote_prepared_audio_runner=lambda audio, sample_rate_hz, num_speakers: _run_prepared(
            audio,
            sample_rate_hz,
            num_speakers,
            "pyannote",
        ),
        nemo_prepared_audio_runner=lambda audio, sample_rate_hz, num_speakers: _run_prepared(
            audio,
            sample_rate_hz,
            num_speakers,
            "nemo",
        ),
    )


def execute_speaker_request_v2(
    request: ExecuteRequestV2,
    host: SpeakerExecutionHostV2,
) -> ExecuteResponseV2:
    """Execute one live V2 speaker request through the Rust control plane."""

    import batchalign_core

    return ExecuteResponseV2.model_validate_json(
        batchalign_core.execute_speaker_request_v2(
            request,
            host.pyannote_prepared_audio_runner,
            host.nemo_prepared_audio_runner,
        )
    )


__all__ = [
    "SpeakerExecutionHostV2",
    "build_default_speaker_execution_host_v2",
    "execute_speaker_request_v2",
]
