"""`Task` — the registerable component pushed onto `Interface`.

One `Task` per pipeline input. The `Interface` consumes the public
mutation API (`start`, `stage_started`, `update`, `complete`, `fail`,
`skip`) to drive rendering; the `bridge` module drives those calls
from Rust-side `ProgressEvent`s.

Invariant: `state` only advances WAIT → RUN → {OK, FAIL, SKIP}. Any
mutation that would move backwards or out of a terminal state raises
`TaskStateError`. The bridge demotes that to a logged warning so a
buggy event stream never poisons the run.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path
from time import monotonic
from typing import Any


class TaskState(Enum):
    WAIT = "WAIT"
    RUN = "RUN"
    OK = "DONE"
    FAIL = "FAIL"
    SKIP = "SKIP"


_TERMINAL = {TaskState.OK, TaskState.FAIL, TaskState.SKIP}


class TaskStateError(RuntimeError):
    """Raised on an illegal state transition; bridge catches + logs."""


@dataclass
class Task:
    """One pipeline input's display + lifecycle state."""

    source_id: str
    label: str
    state: TaskState = TaskState.WAIT
    stage: str | None = None
    progress: tuple[int, int] | None = None
    started: float | None = None
    finished: float | None = None
    error: str | None = None

    @classmethod
    def from_input(cls, inp: Any) -> "Task":
        """Build a Task from a `ChatInput` / `MediaInput` / `PairedInput`.

        The display label is the source path's basename; the
        `source_id` matches what the Rust `Pipeline.run` callback list
        keys on (set by `_common.collect_*_inputs` to the absolute
        source path).
        """
        sid = getattr(inp, "source_id", None) or str(getattr(inp, "path", inp))
        label_src = getattr(inp, "path", None) or getattr(inp, "main", None) or sid
        return cls(source_id=str(sid), label=Path(str(label_src)).name)

    # ----- lifecycle -------------------------------------------------------

    def start(self) -> None:
        if self.state is TaskState.RUN:
            return
        if self.state is not TaskState.WAIT:
            raise TaskStateError(f"start() from {self.state.value}")
        self.state = TaskState.RUN
        self.started = monotonic()

    def stage_started(self, stage: str) -> None:
        if self.state is TaskState.WAIT:
            self.start()
        if self.state is not TaskState.RUN:
            raise TaskStateError(f"stage_started() from {self.state.value}")
        self.stage = stage
        self.progress = None

    def update(self, completed: int, total: int) -> None:
        if self.state is not TaskState.RUN:
            return
        if total <= 0:
            return
        self.progress = (max(0, completed), total)

    def complete(self) -> None:
        if self.state in _TERMINAL:
            return
        self.state = TaskState.OK
        self.finished = monotonic()
        self.progress = None

    def fail(self, error: str) -> None:
        if self.state in _TERMINAL:
            return
        self.state = TaskState.FAIL
        self.finished = monotonic()
        self.error = error or "<no message>"

    def skip(self, reason: str = "") -> None:
        if self.state in _TERMINAL:
            return
        self.state = TaskState.SKIP
        self.finished = monotonic()
        self.error = reason or None

    # ----- views -----------------------------------------------------------

    @property
    def elapsed(self) -> float | None:
        if self.started is None:
            return None
        end = self.finished if self.finished is not None else monotonic()
        return max(0.0, end - self.started)

    @property
    def is_terminal(self) -> bool:
        return self.state in _TERMINAL


__all__ = ["Task", "TaskState", "TaskStateError"]
