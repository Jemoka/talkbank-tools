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
import re
from types import SimpleNamespace

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
    assert "ok=2" in out
    assert "parse error: header missing" in out
    assert ui.exit_code == 1


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
