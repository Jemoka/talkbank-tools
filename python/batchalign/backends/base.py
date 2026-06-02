"""Backend ABC and task-marker ABCs.

!!!  HAND-MIRRORED with crates/batchalign/batchalign-core/src/backend.rs  !!!

A backend declares which tasks it services by *inheriting* from the
matching marker ABC. For example::

    class WhisperBackend(ASR, FA):
        ...

This is a type-level declaration. The kernel introspects the MRO via
`declared_tasks()` to know what tasks the backend can handle. Edits
to this contract must be paired with edits to `backend.rs`. See
`spec2.md` §10.1.

When `batchalign._core` is unavailable (fresh clone, no `maturin develop`
run yet), this module provides lightweight stand-ins for `Task` and
`BatchPolicy` so tests can still collect and the marker-ABC logic can
be exercised without the .so.
"""

from __future__ import annotations

from abc import ABC, abstractmethod
from enum import Enum
from typing import Any

# ---------------------------------------------------------------------------
# Pull Task / BatchPolicy from _core if available; otherwise fall back to
# pure-Python stand-ins. The fallback is for test-collection and import
# hygiene only — real pipeline construction goes through the .so.
# ---------------------------------------------------------------------------
try:
    from batchalign._core import Task, BatchPolicy  # type: ignore[attr-defined]
    _USING_CORE = True
except ImportError:
    _USING_CORE = False

    class Task(str, Enum):  # type: ignore[no-redef]
        """Fallback `Task` enum used when `batchalign._core` is not built.

        The real enum lives in Rust; this matches the variant names from
        `spec2.md` §5 so user code that constructs pipelines via Python
        still type-checks.
        """

        Asr = "Asr"
        Fa = "Fa"
        Speaker = "Speaker"
        UtSeg = "UtSeg"
        Morphosyntax = "Morphosyntax"
        Translate = "Translate"
        Coref = "Coref"
        Compare = "Compare"

    class BatchPolicy:  # type: ignore[no-redef]
        """Fallback `BatchPolicy` used when `batchalign._core` is not built."""

        __slots__ = ("max_size", "window_ms")

        def __init__(self, max_size: int = 32, window_ms: int = 50) -> None:
            self.max_size = max_size
            self.window_ms = window_ms

        @staticmethod
        def one() -> "BatchPolicy":
            return BatchPolicy(max_size=1, window_ms=0)

        @staticmethod
        def fixed(n: int) -> "BatchPolicy":
            return BatchPolicy(max_size=n, window_ms=50)

        def __repr__(self) -> str:  # pragma: no cover
            return f"BatchPolicy(max_size={self.max_size}, window_ms={self.window_ms})"


# ---------------------------------------------------------------------------
# Root Backend ABC.
# ---------------------------------------------------------------------------


class Backend(ABC):
    """Root ABC for all backends.

    Subclasses declare *which tasks they service* by also inheriting from
    one or more task-marker ABCs (`ASR`, `FA`, `Speaker`, ...). The kernel
    reads the MRO to determine the task set; this is exactly the inheritance
    semantic we want.

    Concrete subclasses must implement `name`, `batch_policy`, and `call`.
    """

    @property
    @abstractmethod
    def name(self) -> str:
        """Backend identity used as cache namespace.

        Different model versions MUST yield different names — `whisper:large-v3`
        vs `whisper:medium` would be distinct. Forgetting to bump on a model
        swap silently returns stale cached results.
        """

    @property
    @abstractmethod
    def batch_policy(self) -> BatchPolicy:
        """Preferred batching window. Whisper likes 8-32, Stanza 32-128,
        atomic-call backends like Rev.AI use `BatchPolicy.one()`."""

    @abstractmethod
    def call(self, batch: list[Any], *, progress: Any = None, **_kwargs: Any) -> list[Any]:
        """Run inference on a batch of `TaskInput` variants.

        Output at index `i` MUST correspond to input at index `i` and
        carry the matching output variant. Backends pattern-match on the
        input variant type to dispatch internally.

        ``progress`` is an optional callable supplied by the Rust engine
        (``ScaledProgress`` on the runner side). Backends with an internal
        loop — audio-chunk grouping in FA, multi-stage ASR decoding, etc. —
        SHOULD invoke ``progress(completed, total)`` at meaningful
        increments so the per-file progress bar advances inside the
        single bulk call. The wrapper rescales those ticks into the
        outer runner's bar (see ``crates/batchalign/batchalign-core/
        src/base.rs::ScaledProgress``).

        Backends with nothing useful to report just ignore the parameter.
        ``progress`` may be ``None`` when the runner doesn't supply one
        (legacy ``dispatch`` path); guard with ``if progress: progress(...)``.

        Do NOT stash ``progress`` past the return of ``call`` — the
        callable's lifetime is bounded by the dispatch.

        Extra ``**_kwargs`` is a forward-compat hatch: future runner
        features may pass additional keyword args, and base behaviour is
        to ignore unknown names rather than break the call.
        """


# ---------------------------------------------------------------------------
# Task-marker ABCs.
# Pure tagging — a backend declares which tasks it services by inheriting
# from the matching markers. `class WhisperBackend(ASR, FA): ...` declares
# both. The kernel introspects MRO via `declared_tasks()`.
# ---------------------------------------------------------------------------


class ASR(Backend):
    """Marker: this backend handles `Task.Asr` inputs."""


class FA(Backend):
    """Marker: this backend handles `Task.Fa` (forced alignment) inputs."""


class Speaker(Backend):
    """Marker: this backend handles `Task.Speaker` (diarization) inputs."""


class UtSeg(Backend):
    """Marker: this backend handles `Task.UtSeg` (utterance segmentation) inputs."""


class UTR(Backend):
    """Marker: this backend handles `Task.Utr` (Utterance Timing Recovery) inputs.

    UTR's wire payload is byte-identical to ASR's — the Rust-side proto
    `UtrInput` is a serde-transparent newtype over `AsrInput`. So any ASR
    backend can opt into UTR by adding this marker to its bases:

        class WhisperBackend(ASR, UTR):
            ...

    No `call()` changes are needed; the backend pattern-matches on the same
    `AsrInput` dataclass it already handles. The Rust UTR taskrunner runs
    the Hirschberg strategy over the returned tokens.
    """


class Morphosyntax(Backend):
    """Marker: this backend handles `Task.Morphosyntax` inputs."""


class Translate(Backend):
    """Marker: this backend handles `Task.Translate` inputs."""


class Coref(Backend):
    """Marker: this backend handles `Task.Coref` inputs."""


# ---------------------------------------------------------------------------
# MRO introspection: map marker ABC -> Task variant.
# ---------------------------------------------------------------------------

_TASK_BY_ABC: dict[type, Task] = {
    ASR: Task.Asr,
    FA: Task.Fa,
    Speaker: Task.Speaker,
    UtSeg: Task.UtSeg,
    UTR: Task.Utr,
    Morphosyntax: Task.Morphosyntax,
    Translate: Task.Translate,
    Coref: Task.Coref,
}


def declared_tasks(backend: Backend) -> list[Task]:
    """Return the list of `Task` variants this backend declares.

    Computed from the MRO: every marker ABC the backend inherits from
    contributes its corresponding `Task`. Order follows declaration order
    of `_TASK_BY_ABC` for stability.
    """
    return [t for cls, t in _TASK_BY_ABC.items() if isinstance(backend, cls)]


__all__ = [
    "Backend",
    "ASR",
    "FA",
    "Speaker",
    "UtSeg",
    "Morphosyntax",
    "Translate",
    "Coref",
    "Task",
    "BatchPolicy",
    "declared_tasks",
]
