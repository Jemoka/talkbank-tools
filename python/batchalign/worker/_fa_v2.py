"""Thin worker-protocol V2 forced-alignment wrapper.

Rust now owns the prepared-artifact reads, request validation, backend dispatch,
and typed V2 response shaping for the worker FA boundary. Python stays only at
the model-host callback edge.

**See also:** `../../INTERFACE_MAP.md` section "3. Forced Alignment V2" for:
- Rust FFI function: `crates/batchalign-pyo3/src/worker_fa_exec.rs::execute_forced_alignment_request_v2()`
- Full Rust/Python responsibility split and input/output contracts.

Concurrency note
----------------
The GPU worker serves V2 requests via ``_serve_stdio_concurrent(max_threads=4)``,
which dispatches up to 4 requests simultaneously through a ``ThreadPoolExecutor``.
PyTorch releases the GIL during compute, so threads truly run in parallel on the
CPU side.  However, the torchaudio MMS_FA / ``forced_align`` kernel is **not
thread-safe** for concurrent CPU execution from multiple threads — concurrent
calls cause SIGSEGV/SIGABRT in LibTorch, crashing the entire worker process and
failing every pending request in the job.

``_fa_inference_lock`` serializes all calls to ``execute_forced_alignment_request_v2``
so that only one FA inference runs at a time.  The lock is module-level (one per
worker process), which is the correct granularity — each GPU worker process is a
single Python interpreter serving one language's models.
"""

from __future__ import annotations

import threading
from dataclasses import dataclass
from typing import TYPE_CHECKING

import numpy as np
from pydantic import BaseModel

from batchalign.worker._types_v2 import ExecuteRequestV2, ExecuteResponseV2, ForcedAlignmentRequestV2

if TYPE_CHECKING:
    from collections.abc import Callable

    from batchalign.inference.types import Wave2VecFAHandle, WhisperFAHandle

# Serialize all FA inference calls within a single worker process.
#
# The torchaudio MMS_FA forced_align kernel is not thread-safe for concurrent
# CPU execution. _serve_stdio_concurrent runs a ThreadPoolExecutor(max_threads=4)
# and PyTorch releases the GIL during compute, so without this lock up to 4
# threads can race inside the native C++ kernel simultaneously, causing
# SIGSEGV/SIGABRT. One lock per process is the right granularity because each
# worker process owns exactly one set of loaded models for one language.
_fa_inference_lock = threading.Lock()


class PreparedFaPayloadV2(BaseModel):
    """Prepared FA payload written by the Rust-side V2 request builder."""

    words: list[str]
    word_ids: list[str]
    word_utterance_indices: list[int]
    word_utterance_word_indices: list[int]


@dataclass(frozen=True, slots=True)
class ForcedAlignmentExecutionHostV2:
    """Injected FA execution hooks for the live V2 path."""

    whisper_runner: Callable[[np.ndarray, str, bool], list[tuple[str, float]]] | None = None
    wave2vec_runner: Callable[[np.ndarray, list[str]], list[tuple[str, tuple[int, int]]]] | None = None
    canto_runner: (
        Callable[
            [np.ndarray, PreparedFaPayloadV2, ForcedAlignmentRequestV2],
            list[tuple[str, tuple[int, int]]],
        ]
        | None
    ) = None


def build_default_fa_execution_host_v2(
    *,
    whisper_model: WhisperFAHandle | None,
    wave2vec_model: Wave2VecFAHandle | None,
) -> ForcedAlignmentExecutionHostV2:
    """Build the live V2 FA host from already loaded model handles."""

    from batchalign.inference.fa import infer_wave2vec_fa, infer_whisper_fa
    import torch

    def _as_tensor(audio: np.ndarray) -> torch.Tensor:
        return torch.from_numpy(np.asarray(audio, dtype=np.float32))

    whisper_runner = None
    if whisper_model is not None:

        def _run_whisper(audio: np.ndarray, text: str, pauses: bool) -> list[tuple[str, float]]:
            return infer_whisper_fa(whisper_model, _as_tensor(audio), text, pauses=pauses)

        whisper_runner = _run_whisper

    wave2vec_runner = None
    if wave2vec_model is not None:

        def _run_wave2vec(
            audio: np.ndarray,
            words: list[str],
        ) -> list[tuple[str, tuple[int, int]]]:
            return infer_wave2vec_fa(wave2vec_model, _as_tensor(audio), words)

        wave2vec_runner = _run_wave2vec

    return ForcedAlignmentExecutionHostV2(
        whisper_runner=whisper_runner,
        wave2vec_runner=wave2vec_runner,
    )


def _wrap_canto_runner(
    runner: Callable[[np.ndarray, PreparedFaPayloadV2, ForcedAlignmentRequestV2], object] | None,
) -> Callable[[np.ndarray, str, str], object] | None:
    """Adapt the typed Cantonese host hook to the Rust bridge shape."""

    if runner is None:
        return None

    def _run(audio: np.ndarray, payload_json: str, request_json: str) -> object:
        return runner(
            audio,
            PreparedFaPayloadV2.model_validate_json(payload_json),
            ForcedAlignmentRequestV2.model_validate_json(request_json),
        )

    return _run


def execute_forced_alignment_request_v2(
    request: ExecuteRequestV2,
    host: ForcedAlignmentExecutionHostV2,
) -> ExecuteResponseV2:
    """Execute one live V2 forced-alignment request through the Rust bridge.

    The ``_fa_inference_lock`` is held for the duration of this call to
    prevent concurrent FA inference within the same worker process.  See the
    module-level docstring for the full rationale.
    """
    import batchalign_core

    with _fa_inference_lock:
        return ExecuteResponseV2.model_validate_json(
            batchalign_core.execute_forced_alignment_request_v2(
                request,
                host.whisper_runner,
                host.wave2vec_runner,
                _wrap_canto_runner(host.canto_runner),
            )
        )
