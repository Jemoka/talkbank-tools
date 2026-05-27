"""TUI presentation layer for the batchalign CLI.

`Interface` is the live registry + renderer. `Task` is the per-input
registerable component (state machine: WAIT → RUN → OK/FAIL/SKIP).
Both are imported by every CLI command module; nothing else from
this package should leak into the command files.

The pipeline-event bridge lives in `bridge.py`; it converts Rust
`ProgressEvent`s into `Task` mutations and is wired in via
`Interface.callbacks_for(tasks)`.
"""

from __future__ import annotations

from .interface import Interface
from .task import Task, TaskState, TaskStateError

__all__ = ["Interface", "Task", "TaskState", "TaskStateError"]
