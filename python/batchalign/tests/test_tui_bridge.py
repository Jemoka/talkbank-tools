"""ProgressEvent → Task dispatch.

We install a fake `batchalign._core` via the `fake_progress_core`
conftest fixture so this test runs without the compiled .so.
"""

from __future__ import annotations

from batchalign.cli.tui.bridge import callbacks_for
from batchalign.cli.tui.task import Task, TaskState


def test_stage_started_advances_to_run_and_sets_stage(fake_progress_core):
    RustTask, ProgressKind, ProgressEvent = fake_progress_core
    t = Task(source_id="s", label="a.wav")
    cbs = dict(callbacks_for({"s": t}))
    cbs["s"](ProgressEvent(source_id="s", kind=ProgressKind.StageStarted,
                            task=RustTask.Asr))
    assert t.state is TaskState.RUN
    assert t.stage == "asr"


def test_completed_total_drive_progress(fake_progress_core):
    RustTask, ProgressKind, ProgressEvent = fake_progress_core
    t = Task(source_id="s", label="a.wav")
    cbs = dict(callbacks_for({"s": t}))
    cbs["s"](ProgressEvent(source_id="s", kind=ProgressKind.StageStarted,
                            task=RustTask.Asr))
    cbs["s"](ProgressEvent(source_id="s", kind=ProgressKind.StageStarted,
                            task=RustTask.Asr, completed=12, total=87))
    assert t.progress == (12, 87)


def test_source_completed_advances_to_ok(fake_progress_core):
    RustTask, ProgressKind, ProgressEvent = fake_progress_core
    t = Task(source_id="s", label="a.wav")
    cbs = dict(callbacks_for({"s": t}))
    cbs["s"](ProgressEvent(source_id="s", kind=ProgressKind.StageStarted,
                            task=RustTask.Asr))
    cbs["s"](ProgressEvent(source_id="s", kind=ProgressKind.SourceCompleted))
    assert t.state is TaskState.OK


def test_stage_failed_marks_fail_with_label(fake_progress_core):
    RustTask, ProgressKind, ProgressEvent = fake_progress_core
    t = Task(source_id="s", label="a.wav")
    cbs = dict(callbacks_for({"s": t}))
    cbs["s"](ProgressEvent(source_id="s", kind=ProgressKind.StageStarted,
                            task=RustTask.Asr))
    cbs["s"](ProgressEvent(source_id="s", kind=ProgressKind.StageFailed,
                            task=RustTask.Asr, label="whisper: cuda OOM"))
    assert t.state is TaskState.FAIL
    assert t.error == "whisper: cuda OOM"


def test_stage_skipped_marks_skip(fake_progress_core):
    RustTask, ProgressKind, ProgressEvent = fake_progress_core
    t = Task(source_id="s", label="a.wav")
    cbs = dict(callbacks_for({"s": t}))
    cbs["s"](ProgressEvent(source_id="s", kind=ProgressKind.StageSkipped,
                            task=RustTask.Asr, label="no audio"))
    assert t.state is TaskState.SKIP
    assert t.error == "no audio"


def test_illegal_transition_is_swallowed(fake_progress_core):
    """A buggy event stream must not poison the run."""
    RustTask, ProgressKind, ProgressEvent = fake_progress_core
    t = Task(source_id="s", label="a.wav")
    cbs = dict(callbacks_for({"s": t}))
    cbs["s"](ProgressEvent(source_id="s", kind=ProgressKind.SourceCompleted))
    cbs["s"](ProgressEvent(source_id="s", kind=ProgressKind.StageStarted,
                            task=RustTask.Asr))
    assert t.state is TaskState.OK   # unchanged


def test_on_event_hook_invoked_with_task(fake_progress_core):
    RustTask, ProgressKind, ProgressEvent = fake_progress_core
    t = Task(source_id="s", label="a.wav")
    seen = []
    cbs = dict(callbacks_for({"s": t}, on_event=lambda ev, task: seen.append(task)))
    cbs["s"](ProgressEvent(source_id="s", kind=ProgressKind.StageStarted,
                            task=RustTask.Asr))
    assert seen == [t]
