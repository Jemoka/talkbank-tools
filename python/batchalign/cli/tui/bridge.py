"""Bridge between Rust `ProgressEvent`s and the `Task` lifecycle.

`callbacks_for(tasks, on_event=...)` returns the list of
`(source_id, callable)` pairs that `Pipeline.run` expects in its
`callbacks=` argument. Each callable inspects the event's
`ProgressKind` and invokes the matching method on the matching
`Task`. Out-of-order or illegal transitions raise inside the `Task`
state machine; we catch + log to keep the run alive (mirrors the
Rust `CallbackSink::emit` doctrine in
`crates/batchalign/batchalign-engine/src/progress_sink.rs:34–46`,
which already swallows callback exceptions on the Rust side).

Stage labels are lower-cased Task enum names (`asr`, `fa`, `morph`,
…), short enough to fit in the per-file progress bar's description
column. The Rust `Task` enum is exposed via `batchalign._core`.
"""

from __future__ import annotations

import logging
from typing import Any, Callable

from .task import Task, TaskStateError


_log = logging.getLogger("batchalign.cli.tui.bridge")


def _stage_label(rust_task: Any | None) -> str:
    """`ProgressEvent.task` is the Rust `Task` enum (or None).

    The PyO3-generated enum's `str()` is `"Task.Morphosyntax"`; its
    `.name` is sometimes absent depending on PyO3 version. We
    canonicalise by taking the segment after the final `.`, which
    works for both the real enum and the test fakes.
    """
    if rust_task is None:
        return "?"
    name = getattr(rust_task, "name", None) or str(rust_task)
    return name.rsplit(".", 1)[-1].lower()


def callbacks_for(
    tasks: dict[str, Task],
    *,
    on_event: Callable[[Any, Task], None] | None = None,
) -> list[tuple[str, Callable[[Any], None]]]:
    """Wire each `source_id` to a closure that mutates its `Task`.

    `on_event`, if supplied, is invoked AFTER the task mutation —
    `Interface` uses it to refresh its Rich Progress bars and to log
    `ProgressEvent` traffic at -vv.
    """
    out: list[tuple[str, Callable[[Any], None]]] = []
    for sid, task in tasks.items():
        out.append((sid, _make_callback(task, on_event)))
    return out


def _make_callback(
    task: Task,
    on_event: Callable[[Any, Task], None] | None,
) -> Callable[[Any], None]:
    # Late import — `ProgressKind` lives in the compiled extension. We
    # import from `batchalign._core` (not `batchalign`) so test code can
    # substitute the module via `sys.modules["batchalign._core"] = fake`
    # without having to retro-patch attributes on `batchalign`.
    from batchalign._core import ProgressKind  # type: ignore[attr-defined]

    def on(ev: Any) -> None:
        try:
            kind = ev.kind
            if kind is ProgressKind.StageStarted:
                task.stage_started(_stage_label(ev.task))
            elif kind is ProgressKind.StageFailed:
                task.fail(ev.label or "<no message>")
            elif kind is ProgressKind.StageSkipped:
                task.skip(ev.label or "")
            elif kind is ProgressKind.SourceCompleted:
                task.complete()
            # StageInjected: stage finished cleanly, no display change
            # (the next StageStarted or SourceCompleted advances state).
            if getattr(ev, "total", 0):
                task.update(int(ev.completed or 0), int(ev.total))
        except TaskStateError as exc:
            _log.warning("ignored illegal transition for %s: %s",
                         task.source_id, exc)
        if on_event is not None:
            try:
                on_event(ev, task)
            except Exception:  # noqa: BLE001
                _log.exception("on_event hook raised for %s", task.source_id)

    return on


__all__ = ["callbacks_for"]
