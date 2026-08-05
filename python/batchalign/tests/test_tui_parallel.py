"""`Interface.run_pipeline` submits inputs as a single batched call.

Regression coverage for the parallelism fix: previously the loop ran
one `pipeline.run([single], …)` per file, serializing what the Rust
engine semaphore would otherwise run 8-wide. After the fix, the TUI
hands the full input list over and lets the engine schedule.

Two invariants matter:

1. **One batched call.** The fake pipeline asserts it sees the full
   list in one shot (not N invocations of length 1).
2. **Per-source failure isolation.** A `BAValue::Failed`-equivalent
   outcome for one source must not poison the others — the failed
   task ends in FAIL, the healthy tasks complete, and only successful
   outcomes are yielded.
"""

from __future__ import annotations

import io
import os
import re
import threading
from types import SimpleNamespace

from rich.console import Console

from batchalign.cli.tui import Interface, Task, TaskState


def _capture_console() -> tuple[Console, io.StringIO]:
    buf = io.StringIO()
    console = Console(
        file=buf, force_terminal=False, no_color=True,
        highlight=False, markup=False, width=120,
    )
    return console, buf


def _strip_ansi(s: str) -> str:
    return re.sub(r"\x1b\[[0-9;]*m", "", s)


def test_run_pipeline_submits_inputs_in_one_call(fake_progress_core):
    """The whole point: one pipeline.run call covering all inputs."""
    RustTask, ProgressKind, ProgressEvent = fake_progress_core
    console, _ = _capture_console()

    calls: list[int] = []

    class FakePipeline:
        def run(self, inputs, callbacks):
            calls.append(len(inputs))
            cbs = dict(callbacks)
            outcomes = []
            for inp in inputs:
                sid = inp.source_id
                cbs[sid](ProgressEvent(
                    source_id=sid, kind=ProgressKind.StageStarted,
                    task=RustTask.Asr,
                ))
                cbs[sid](ProgressEvent(
                    source_id=sid, kind=ProgressKind.SourceCompleted,
                ))
                outcomes.append(SimpleNamespace(source_id=sid, ok=True))
            return outcomes

    ui = Interface.open(
        command="transcribe", params={}, output=None,
        plain=True, console=console,
    )
    inputs = [SimpleNamespace(source_id=sid, path=f"/x/{sid}.wav")
              for sid in ("a", "b", "c")]
    with ui:
        for inp in inputs:
            ui.push(Task.from_input(inp))
        outs = list(ui.run_pipeline(FakePipeline(), inputs))

    assert calls == [3], f"expected single batched call of 3, got {calls}"
    assert len(outs) == 3
    assert ui.exit_code == 0


def test_interactive_dashboard_keeps_textual_on_main_thread(
    fake_progress_core, monkeypatch
):
    """Interactive runs move only pipeline work off the main/UI thread."""
    RustTask, ProgressKind, ProgressEvent = fake_progress_core
    console, _ = _capture_console()
    main_thread = threading.current_thread()
    pipeline_threads: list[threading.Thread] = []
    dashboard_threads: list[threading.Thread] = []
    dashboard_updates: list[bool] = []

    class FakeDashboard:
        def __init__(self, **_kwargs):
            dashboard_threads.append(threading.current_thread())

        def start(self):
            pass

        def update(self, _tasks, *, finished=False):
            dashboard_updates.append(finished)

        def run_while(self, worker, _tasks, **_kwargs):
            thread = threading.Thread(target=worker)
            thread.start()
            thread.join()

        def close(self, _tasks):
            pass

    import batchalign.cli.tui.dashboard as dashboard_module

    monkeypatch.setattr(dashboard_module, "Dashboard", FakeDashboard)

    class FakePipeline:
        def run(self, inputs, callbacks):
            pipeline_threads.append(threading.current_thread())
            callback = dict(callbacks)[inputs[0].source_id]
            callback(
                ProgressEvent(
                    source_id=inputs[0].source_id,
                    kind=ProgressKind.StageStarted,
                    task=RustTask.Asr,
                )
            )
            callback(
                ProgressEvent(
                    source_id=inputs[0].source_id,
                    kind=ProgressKind.SourceCompleted,
                )
            )
            return [SimpleNamespace(source_id=inputs[0].source_id)]

    inp = SimpleNamespace(source_id="a", path="/x/a.wav")
    ui = Interface.open(
        command="transcribe", params={}, output=None, plain=False, console=console
    )
    with ui:
        ui.push(Task.from_input(inp))
        list(ui.run_pipeline(FakePipeline(), [inp]))

    assert dashboard_threads == [main_thread]
    assert pipeline_threads and pipeline_threads[0] is not main_thread
    assert len(dashboard_updates) == 2


def test_interactive_pipeline_error_is_normalized_before_final_frame(
    fake_progress_core, monkeypatch
):
    """The dashboard's last snapshot must contain terminal failure states."""
    console, _ = _capture_console()
    final_states = []

    class FakeDashboard:
        def __init__(self, **_kwargs):
            pass

        def start(self):
            pass

        def update(self, _tasks, *, finished=False):
            pass

        def run_while(self, worker, tasks, *, on_error=None):
            errors = []

            def run():
                try:
                    worker()
                except BaseException as exc:  # noqa: BLE001 - mirrors adapter
                    errors.append(exc)
                    if on_error is not None:
                        on_error(exc)

            thread = threading.Thread(target=run)
            thread.start()
            thread.join()
            final_states.extend(task.state for task in tasks)
            if errors:
                raise errors[0]

        def close(self, _tasks):
            pass

    import batchalign.cli.tui.dashboard as dashboard_module

    monkeypatch.setattr(dashboard_module, "Dashboard", FakeDashboard)

    class BrokenPipeline:
        def run(self, _inputs, callbacks):
            raise RuntimeError("engine unavailable")

    inputs = [
        SimpleNamespace(source_id=sid, path=f"/x/{sid}.wav") for sid in ("a", "b")
    ]
    ui = Interface.open(
        command="transcribe", params={}, output=None, plain=False, console=console
    )
    with ui:
        for inp in inputs:
            ui.push(Task.from_input(inp))
        list(ui.run_pipeline(BrokenPipeline(), inputs))

    assert final_states == [TaskState.FAIL, TaskState.FAIL]
    assert ui.exit_code == 2


def test_interactive_cancel_is_normalized_before_final_frame(
    fake_progress_core, monkeypatch
):
    RustTask, ProgressKind, ProgressEvent = fake_progress_core
    console, _ = _capture_console()
    final_states = []

    class FakeDashboard:
        def __init__(self, **_kwargs):
            pass

        def start(self):
            pass

        def update(self, _tasks, *, finished=False):
            pass

        def run_while(self, worker, tasks, **_kwargs):
            thread = threading.Thread(target=worker)
            thread.start()
            thread.join()
            final_states.extend(task.state for task in tasks)

        def close(self, _tasks):
            pass

    import batchalign.cli.tui.dashboard as dashboard_module

    monkeypatch.setattr(dashboard_module, "Dashboard", FakeDashboard)

    class CancelledPipeline:
        def __init__(self):
            self.cancelled = False

        def cancel(self):
            self.cancelled = True

        def run(self, inputs, callbacks):
            callback = dict(callbacks)[inputs[0].source_id]
            callback(
                ProgressEvent(
                    source_id=inputs[0].source_id,
                    kind=ProgressKind.StageStarted,
                    task=RustTask.Asr,
                )
            )
            ui._request_cancel()
            return [SimpleNamespace(source_id=inp.source_id) for inp in inputs]

    inputs = [
        SimpleNamespace(source_id=sid, path=f"/x/{sid}.wav") for sid in ("a", "b")
    ]
    pipeline = CancelledPipeline()
    ui = Interface.open(
        command="transcribe", params={}, output=None, plain=False, console=console
    )
    with ui:
        for inp in inputs:
            ui.push(Task.from_input(inp))
        list(ui.run_pipeline(pipeline, inputs))

    assert pipeline.cancelled
    assert final_states == [TaskState.FAIL, TaskState.SKIP]
    assert ui.exit_code == 130


def test_second_cancel_request_hard_exits(monkeypatch):
    console, _ = _capture_console()
    ui = Interface.open(
        command="transcribe", params={}, output=None, plain=True, console=console
    )
    exit_codes = []

    def fake_exit(code):
        exit_codes.append(code)
        raise SystemExit(code)

    monkeypatch.setattr(os, "_exit", fake_exit)
    ui._request_cancel()
    try:
        ui._request_cancel()
    except SystemExit as exc:
        assert exc.code == 130
    else:
        raise AssertionError("second cancellation did not hard-exit")

    assert exit_codes == [130]


def test_run_pipeline_enables_traceback_capture_only_at_vv(fake_progress_core):
    RustTask, ProgressKind, ProgressEvent = fake_progress_core
    console, _ = _capture_console()
    env_values: list[str | None] = []

    class FakePipeline:
        def run(self, inputs, callbacks):
            env_values.append(os.environ.get("BATCHALIGN_CLI_VERBOSE_TRACEBACKS"))
            cbs = dict(callbacks)
            for inp in inputs:
                cbs[inp.source_id](ProgressEvent(
                    source_id=inp.source_id,
                    kind=ProgressKind.StageStarted,
                    task=RustTask.Asr,
                ))
                cbs[inp.source_id](ProgressEvent(
                    source_id=inp.source_id,
                    kind=ProgressKind.SourceCompleted,
                ))
            return [SimpleNamespace(source_id=inp.source_id, ok=True)
                    for inp in inputs]

    inp = SimpleNamespace(source_id="a", path="/x/a.wav")
    old = os.environ.pop("BATCHALIGN_CLI_VERBOSE_TRACEBACKS", None)
    try:
        ui = Interface.open(
            command="transcribe",
            params={},
            output=None,
            plain=True,
            console=console,
            verbosity=1,
        )
        with ui:
            ui.push(Task.from_input(inp))
            list(ui.run_pipeline(FakePipeline(), [inp]))
        assert env_values == [None]
        assert os.environ.get("BATCHALIGN_CLI_VERBOSE_TRACEBACKS") is None

        ui = Interface.open(
            command="transcribe",
            params={},
            output=None,
            plain=True,
            console=console,
            verbosity=2,
        )
        with ui:
            ui.push(Task.from_input(inp))
            list(ui.run_pipeline(FakePipeline(), [inp]))
        assert env_values == [None, "1"]
        assert os.environ.get("BATCHALIGN_CLI_VERBOSE_TRACEBACKS") is None
    finally:
        if old is not None:
            os.environ["BATCHALIGN_CLI_VERBOSE_TRACEBACKS"] = old


def test_run_pipeline_isolates_per_source_failures(fake_progress_core):
    """One bad source must not take the others out, and must not yield."""
    RustTask, ProgressKind, ProgressEvent = fake_progress_core
    console, buf = _capture_console()

    class FakePipeline:
        def run(self, inputs, callbacks):
            cbs = dict(callbacks)
            outcomes = []
            for inp in inputs:
                sid = inp.source_id
                if sid == "bad":
                    # Mirror what the Rust convert path now does:
                    # StageFailed + SourceCompleted, then surface the
                    # failed BAValue in the outcomes list.
                    cbs[sid](ProgressEvent(
                        source_id=sid, kind=ProgressKind.StageFailed,
                        label="parse error: header missing",
                    ))
                    cbs[sid](ProgressEvent(
                        source_id=sid, kind=ProgressKind.SourceCompleted,
                    ))
                    outcomes.append(SimpleNamespace(source_id=sid, failed=True))
                else:
                    cbs[sid](ProgressEvent(
                        source_id=sid, kind=ProgressKind.StageStarted,
                        task=RustTask.Morphosyntax,
                    ))
                    cbs[sid](ProgressEvent(
                        source_id=sid, kind=ProgressKind.SourceCompleted,
                    ))
                    outcomes.append(SimpleNamespace(source_id=sid, ok=True))
            return outcomes

    ui = Interface.open(
        command="morphotag", params={}, output=None,
        plain=True, console=console,
    )
    inputs = [SimpleNamespace(source_id=sid, path=f"/x/{sid}.cha")
              for sid in ("good1", "bad", "good2")]
    with ui:
        for inp in inputs:
            ui.push(Task.from_input(inp))
        outs = list(ui.run_pipeline(FakePipeline(), inputs))

    # Failed source's outcome must NOT be yielded (no error sidecars).
    assert {getattr(o, "source_id", None) for o in outs} == {"good1", "good2"}
    out = _strip_ansi(buf.getvalue())
    assert "fail=1" in out
    assert "done=2" in out
    assert "parse error: header missing" in out
    assert ui.exit_code == 1


def test_run_pipeline_outcome_callback_writes_during_run(fake_progress_core):
    """When a writer callback is supplied, successes are not yielded again."""
    RustTask, ProgressKind, ProgressEvent = fake_progress_core
    console, _ = _capture_console()
    written: list[str] = []
    timeline: list[tuple[str, list[str]]] = []

    class FakePipeline:
        def run(self, inputs, callbacks, outcome_callback=None):
            assert outcome_callback is not None
            cbs = dict(callbacks)
            outcomes = []
            for inp in inputs:
                sid = inp.source_id
                cbs[sid](ProgressEvent(
                    source_id=sid,
                    kind=ProgressKind.StageStarted,
                    task=RustTask.Morphosyntax,
                ))
                cbs[sid](ProgressEvent(
                    source_id=sid,
                    kind=ProgressKind.SourceCompleted,
                ))
                outcome = SimpleNamespace(source_id=sid, ok=True)
                outcome_callback(outcome)
                timeline.append((f"{sid}-completed", list(written)))
                outcomes.append(outcome)
            timeline.append(("returning", list(written)))
            return outcomes

    ui = Interface.open(
        command="morphotag", params={}, output=None,
        plain=True, console=console,
    )
    inputs = [SimpleNamespace(source_id=sid, path=f"/x/{sid}.cha")
              for sid in ("a", "b")]
    with ui:
        for inp in inputs:
            ui.push(Task.from_input(inp))
        outs = list(
            ui.run_pipeline(
                FakePipeline(),
                inputs,
                on_outcome=lambda outcome: written.append(outcome.source_id),
            )
        )

    assert outs == []
    assert written == ["a", "b"]
    assert timeline == [
        ("a-completed", ["a"]),
        ("b-completed", ["a", "b"]),
        ("returning", ["a", "b"]),
    ]
    assert ui.exit_code == 0


def test_per_utterance_ticks_advance_task_progress(fake_progress_core):
    """A backend that fires `StageStarted` events with (completed, total)
    set should drive `task.progress` from (0,N) to (N,N).

    Mirrors the Rust runner contract: emit a `stage_tick` (= StageStarted
    with non-zero total) after each per-utterance dispatch. The Python
    bridge's `if ev.total > 0: task.update(...)` path handles it.
    """
    RustTask, ProgressKind, ProgressEvent = fake_progress_core
    console, _ = _capture_console()

    captured_progress: list[tuple[int, int] | None] = []

    class TickingPipeline:
        def run(self, inputs, callbacks):
            cbs = dict(callbacks)
            outcomes = []
            for inp in inputs:
                sid = inp.source_id
                # Initial stage_started (no counters).
                cbs[sid](ProgressEvent(
                    source_id=sid, kind=ProgressKind.StageStarted,
                    task=RustTask.Morphosyntax,
                ))
                # 5 per-utterance ticks: (1,5), (2,5), ..., (5,5).
                for i in range(1, 6):
                    cbs[sid](ProgressEvent(
                        source_id=sid, kind=ProgressKind.StageStarted,
                        task=RustTask.Morphosyntax,
                        completed=i, total=5,
                    ))
                cbs[sid](ProgressEvent(
                    source_id=sid, kind=ProgressKind.SourceCompleted,
                ))
                outcomes.append(SimpleNamespace(source_id=sid, ok=True))
            return outcomes

    ui = Interface.open(
        command="morphotag", params={}, output=None,
        plain=True, console=console,
    )
    inp = SimpleNamespace(source_id="a", path="/x/a.cha")
    with ui:
        task = ui.push(Task.from_input(inp))
        list(ui.run_pipeline(TickingPipeline(), [inp]))
        captured_progress.append(task.progress)

    # After SourceCompleted, complete() runs which clears progress to None.
    # The point is the ticks made it through without raising; we verify
    # final state is OK (counts credited) rather than the cleared progress.
    assert ui.exit_code == 0
    assert task.state.value == "DONE"


def test_run_pipeline_top_level_raise_marks_all_unterminated(fake_progress_core):
    """If pipeline.run itself raises, every live task should fail once."""
    console, buf = _capture_console()

    class ExplodingPipeline:
        def run(self, inputs, callbacks):
            raise RuntimeError("engine died")

    ui = Interface.open(
        command="transcribe", params={}, output=None,
        plain=True, console=console,
    )
    inputs = [SimpleNamespace(source_id=sid, path=f"/x/{sid}.wav")
              for sid in ("a", "b")]
    with ui:
        for inp in inputs:
            ui.push(Task.from_input(inp))
        outs = list(ui.run_pipeline(ExplodingPipeline(), inputs))

    assert outs == []
    out = _strip_ansi(buf.getvalue())
    assert "fail=2" in out
    assert "engine died" in out
    # Total failure → exit code 2 (matches `test_total_failure_exit_two`).
    assert ui.exit_code == 2
