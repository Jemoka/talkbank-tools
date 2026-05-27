"""End-to-end Interface lifecycle in plain mode.

We construct a Rich Console writing to a StringIO so the output is
captured deterministically. Drives the task lifecycle directly
(bypassing the Rust pipeline) by routing synthetic ProgressEvents
through the bridge — the same path the real pipeline would take.

The `fake_progress_core` fixture (in conftest.py) installs a fake
`batchalign._core` so this test runs without the compiled .so.
"""

from __future__ import annotations

import io
import re

from rich.console import Console

from batchalign.cli.tui import Interface, Task


def _capture_console() -> tuple[Console, io.StringIO]:
    buf = io.StringIO()
    console = Console(
        file=buf, force_terminal=False, no_color=True,
        highlight=False, markup=False, width=120,
    )
    return console, buf


def _strip_ansi(s: str) -> str:
    return re.sub(r"\x1b\[[0-9;]*m", "", s)


def test_all_ok_exit_zero_and_done_line(fake_progress_core):
    RustTask, ProgressKind, ProgressEvent = fake_progress_core
    console, buf = _capture_console()
    ui = Interface.open(
        command="transcribe",
        params={"asr": "whisper", "lang": "en"},
        output=None,
        plain=True,
        console=console,
    )
    with ui:
        a = ui.push(Task(source_id="a", label="01.wav"))
        b = ui.push(Task(source_id="b", label="02.wav"))
        cbs = dict(ui.callbacks_for({"a": a, "b": b}))
        for sid in ("a", "b"):
            cbs[sid](ProgressEvent(source_id=sid,
                                    kind=ProgressKind.StageStarted,
                                    task=RustTask.Asr))
            cbs[sid](ProgressEvent(source_id=sid,
                                    kind=ProgressKind.SourceCompleted))
    out = _strip_ansi(buf.getvalue())
    assert ui.exit_code == 0
    assert "batchalign3 transcribe" in out
    assert "2 files" in out
    assert "ok=2" in out
    assert "fail=0" in out
    assert "ok  " in out  # at least one completion line


def test_partial_failure_exit_one_and_hint_in_summary(fake_progress_core):
    RustTask, ProgressKind, ProgressEvent = fake_progress_core
    console, buf = _capture_console()
    ui = Interface.open(
        command="transcribe",
        params={"asr": "whisper"},
        output=None,
        plain=True,
        console=console,
    )
    with ui:
        a = ui.push(Task(source_id="a", label="01.wav"))
        b = ui.push(Task(source_id="b", label="02.wav"))
        cbs = dict(ui.callbacks_for({"a": a, "b": b}))
        cbs["a"](ProgressEvent(source_id="a",
                                kind=ProgressKind.StageStarted,
                                task=RustTask.Asr))
        cbs["a"](ProgressEvent(source_id="a",
                                kind=ProgressKind.SourceCompleted))
        cbs["b"](ProgressEvent(source_id="b",
                                kind=ProgressKind.StageStarted,
                                task=RustTask.Asr))
        cbs["b"](ProgressEvent(source_id="b",
                                kind=ProgressKind.StageFailed,
                                task=RustTask.Asr,
                                label="whisper: cuda OOM"))
    out = _strip_ansi(buf.getvalue())
    assert ui.exit_code == 1
    assert "fail=1" in out
    assert "ok=1" in out
    assert "whisper: cuda OOM" in out
    assert "hint: try --device cpu" in out


def test_total_failure_exit_two(fake_progress_core):
    RustTask, ProgressKind, ProgressEvent = fake_progress_core
    console, _buf = _capture_console()
    ui = Interface.open(
        command="transcribe",
        params={},
        output=None,
        plain=True,
        console=console,
    )
    with ui:
        a = ui.push(Task(source_id="a", label="x.wav"))
        cbs = dict(ui.callbacks_for({"a": a}))
        cbs["a"](ProgressEvent(source_id="a",
                                kind=ProgressKind.StageStarted,
                                task=RustTask.Asr))
        cbs["a"](ProgressEvent(source_id="a",
                                kind=ProgressKind.StageFailed,
                                task=RustTask.Asr,
                                label="boom"))
    assert ui.exit_code == 2


def test_empty_inputs_exit_two_setup_failure(fake_progress_core):
    console, _buf = _capture_console()
    ui = Interface.open(
        command="transcribe",
        params={},
        output=None,
        plain=True,
        console=console,
    )
    with ui:
        pass  # zero pushes
    assert ui.exit_code == 2


def test_exception_in_block_marks_running_tasks_fail(fake_progress_core):
    RustTask, ProgressKind, ProgressEvent = fake_progress_core
    console, buf = _capture_console()
    ui = Interface.open(
        command="transcribe",
        params={},
        output=None,
        plain=True,
        console=console,
    )
    try:
        with ui:
            a = ui.push(Task(source_id="a", label="x.wav"))
            cbs = dict(ui.callbacks_for({"a": a}))
            cbs["a"](ProgressEvent(source_id="a",
                                    kind=ProgressKind.StageStarted,
                                    task=RustTask.Asr))
            raise RuntimeError("kaboom")
    except RuntimeError:
        pass
    out = _strip_ansi(buf.getvalue())
    assert ui.exit_code == 2  # all tasks failed
    assert "kaboom" in out
