"""`Interface` — the live registry that owns the deck and the summary.

Lifecycle inside a CLI command:

    with Interface.open(command=..., params=..., output=...) as ui:
        # heavy backend constructors run here — the user sees the
        # "preparing pipeline…" spinner from `__enter__` below.
        pipeline = ba.recipes.foo(...)
        inputs, root = collect_*_inputs(folder)
        tasks = {str(inp.source_id): ui.push(Task.from_input(inp)) for inp in inputs}
        outcomes = pipeline.run(inputs, callbacks=ui.callbacks_for(tasks))
        write_outcomes(...)
    raise typer.Exit(code=ui.exit_code)

The deck is the single source of truth. There is ONE Rich `Progress`
region with:
  - an overall bar (in batch mode), plus
  - one bar per pushed file, pre-registered in WAIT state.

As `ProgressEvent`s flow in we mutate each bar's status column
(wait / run / done / fail / skip) and its (completed, total) counts.
Bars stay visible after the run so the final state of every file is
on screen. The summary block prints once below, with failure
details + hints. **No scrolling completion log** — that would
duplicate what the bars already say.

Plain mode (non-TTY or `--plain`) replaces the live deck with one
column-aligned line per state transition, deduplicated against the
summary.
"""

from __future__ import annotations

import logging
import os
import signal
import sys
import time
from contextlib import contextmanager, suppress
from pathlib import Path
from typing import Any, Callable

from rich.console import Console
from rich.progress import (
    BarColumn,
    MofNCompleteColumn,
    Progress,
    TaskID,
    TextColumn,
    TimeElapsedColumn,
    TimeRemainingColumn,
)
from rich.status import Status
from rich.syntax import Syntax

from ... import config as _ba_config
from . import bridge
from .errors import (
    extract_verbose_traceback,
    is_rich,
    normalise_one_line,
    render_error,
)
from .hints import hint_for
from .task import Task, TaskState


_log = logging.getLogger("batchalign.cli.tui")


# Column widths — kept here so plain mode and the summary line up.
_W_STATE = 4    # "done" / "fail" / "skip" / "wait" / "run "
_W_LABEL = 44   # filename column
_W_STAGE = 8    # current stage label
_W_TIME = 8

# Status palette — color encodes state, nothing else.
_STATE_STYLE = {
    TaskState.WAIT: "dim",
    TaskState.RUN:  "yellow",
    TaskState.OK:   "green",
    TaskState.FAIL: "red",
    TaskState.SKIP: "blue",
}
_TRACEBACK_ENV = "BATCHALIGN_CLI_VERBOSE_TRACEBACKS"


class Interface:
    """Live registry + renderer. One instance per CLI invocation."""

    def __init__(
        self,
        *,
        command: str,
        params: dict[str, object],
        output: Path | None,
        verbosity: int,
        plain: bool,
        quiet: bool,
        console: Console,
    ) -> None:
        self.command = command
        self.params = params
        self.output = output
        self.verbosity = verbosity
        self.plain = plain
        self.quiet = quiet
        self.console = console

        self._tasks: dict[str, Task] = {}
        self._rich_ids: dict[str, TaskID] = {}      # batch/single: per-task rich id
        self._stage_ids: dict[str, TaskID] = {}     # single-file: stage → rich id
        self._single_prev_stage: str | None = None
        self._overall: TaskID | None = None
        self._completed_count = 0
        self._progress: Progress | None = None
        self._status: Status | None = None
        self._started_at: float = 0.0
        self._sigint_prev: Any = None
        self._interrupted = False
        self._interrupt_at: float = 0.0
        # Active pipeline, set by `callbacks_for` / `run_pipeline` so the
        # SIGINT handler can flip its cooperative-cancel flag. Without
        # this, Ctrl-C just sets a Python-side flag that doesn't fire
        # until `pipeline.run` returns — which is exactly the slow-to-die
        # behavior we want to fix.
        self._pipeline: Any = None
        self._opened = False
        # Plain-mode bookkeeping: which (source_id, kind) lines have we
        # already emitted, to avoid duplicate `start` / `done` rows.
        self._plain_started: set[str] = set()
        self._plain_completed: set[str] = set()
        # Sources we've already credited toward the overall completed
        # counter. Each Task can fire multiple terminal-looking events
        # (e.g. StageFailed followed by SourceCompleted); without this
        # guard the overall bar runs past the total ("3/2").
        self._counted_complete: set[str] = set()
        # If the pipeline raises during `run`, we stash the exception
        # message here and surface it ONCE in the summary as a
        # "pipeline aborted" banner — instead of dumping the same text
        # onto every per-file row (which is the duplication the user
        # flagged).
        self._pipeline_error: str | None = None

    # ----- construction ---------------------------------------------------

    @classmethod
    def open(
        cls,
        *,
        command: str,
        params: dict[str, object] | None = None,
        output: Path | None = None,
        verbosity: int = 0,
        plain: bool | None = None,
        quiet: bool = False,
        console: Console | None = None,
    ) -> "Interface":
        no_color = "NO_COLOR" in os.environ
        if plain is None:
            plain = not Console().is_terminal
        # Verbose logging writes to the same stream as the live deck;
        # the Rich Progress region and spinner end up interleaved with
        # log lines and corrupt both. Drop to plain mode whenever the
        # caller asked for verbose output.
        if verbosity > 0:
            plain = True
        if console is None:
            if plain:
                console = Console(
                    file=sys.stdout,
                    force_terminal=False,
                    no_color=True,
                    highlight=False,
                    markup=False,
                )
            else:
                console = Console(no_color=no_color)
        return cls(
            command=command,
            params=params or {},
            output=output,
            verbosity=verbosity,
            plain=bool(plain),
            quiet=quiet,
            console=console,
        )

    # ----- registry -------------------------------------------------------

    def push(self, task: Task) -> Task:
        if task.source_id in self._tasks:
            raise ValueError(f"task {task.source_id!r} already pushed")
        self._tasks[task.source_id] = task
        return task

    def tasks(self) -> list[Task]:
        return list(self._tasks.values())

    def attach_pipeline(self, pipeline: Any) -> None:
        """Tell the interface which pipeline is currently running so the
        SIGINT handler can call its cooperative `cancel()` method.

        Without this, Ctrl-C only fires *after* `pipeline.run` returns:
        the Python signal handler can't run while the Rust runtime holds
        the main thread inside `py.detach`. The Rust side now polls
        signals itself (see `pipeline.rs::run`), but we still call cancel
        from Python so any post-block Ctrl-C also takes effect.
        """
        self._pipeline = pipeline

    def callbacks_for(self, tasks: dict[str, Task]) -> list[tuple[str, Callable]]:
        """Bridge `ProgressEvent`s → `Task` mutations + our refresh hook.

        Side effect: this is the "done pushing, about to run" moment.
        Closes the preparing-spinner and opens the deck, so the user
        sees the full per-file bar list BEFORE `pipeline.run` starts
        emitting events.
        """
        self._open_run()
        return bridge.callbacks_for(tasks, on_event=self._on_event)

    def run_pipeline(
        self,
        pipeline: Any,
        inputs: list[Any],
        *,
        on_outcome: Callable[[Any], None] | None = None,
    ):
        """Submit `inputs` as a single `pipeline.run(inputs, callbacks=…)` call.

        Rust does both the concurrency and the per-backend batching:

        - `pipeline.rs` acquires from a `max_concurrent_values=8`
          semaphore per input future, so submitting N inputs caps
          in-flight work at 8 without any Python-side limiter.
        - Each backend's batcher loop coalesces in-flight calls up to
          `BatchPolicy.max_size` per `window_ms`, which is the whole
          point of batching: Stanza / MT / ASR see one fat call across
          sources, not N tiny ones.

        We previously called `pipeline.run([single], …)` per file to
        contain "1 broken file killed my 50-file run". That isolation
        is now enforced inside `convert_py_input` — a parse failure
        becomes a `BAValue::Failed` for that source and the rest of
        the batch keeps running. So we can hand all inputs over and
        let the engine do its job.

        Yields the outcome of each successfully-processed input.
        Failures land on that input's `Task` directly (via callbacks)
        and on the returned `BAValue::Failed` (filtered out here).
        """
        self._open_run()
        self.attach_pipeline(pipeline)
        tasks_by_sid: dict[str, Task] = {}
        ordered_sids: list[str] = []
        for inp in inputs:
            sid = str(getattr(inp, "source_id", "") or "")
            task = self._tasks.get(sid)
            if task is None:
                task = self.push(Task.from_input(inp))
            tasks_by_sid[sid] = task
            ordered_sids.append(sid)

        cbs = bridge.callbacks_for(tasks_by_sid, on_event=self._on_event)
        old_traceback_env = os.environ.get(_TRACEBACK_ENV)
        if self.verbosity >= 2:
            os.environ[_TRACEBACK_ENV] = "1"
        try:
            if on_outcome is None:
                outcomes = pipeline.run(list(inputs), callbacks=cbs)
            else:
                outcomes = pipeline.run(
                    list(inputs),
                    callbacks=cbs,
                    outcome_callback=on_outcome,
                )
        except Exception as exc:  # noqa: BLE001
            # A pipeline-level raise now means a genuinely unrecoverable
            # error (no source_id could even be derived for one of the
            # inputs, or the engine itself died). Mark every still-live
            # task as failed and surface the banner once.
            self._pipeline_error = f"{type(exc).__name__}: {exc}"
            for task in tasks_by_sid.values():
                if not task.is_terminal:
                    task.fail(self._pipeline_error)
                    if self._progress is not None:
                        self._refresh_task(task)
                    self._mark_completed(task)
            return
        finally:
            if old_traceback_env is None:
                os.environ.pop(_TRACEBACK_ENV, None)
            else:
                os.environ[_TRACEBACK_ENV] = old_traceback_env

        # Outcomes come back in the same order as inputs. Yield only
        # the healthy ones; failed sources already wrote their fail
        # state via callbacks. We don't want write_outcomes to litter
        # the working directory with `<name>.error.log` sidecars when
        # the summary already reports the failure.
        for sid, outcome in zip(ordered_sids, outcomes):
            task = tasks_by_sid.get(sid)
            if task is None:
                if on_outcome is None:
                    yield outcome
                continue
            if task.is_terminal:
                self._mark_completed(task)
            if on_outcome is None and task.state is not TaskState.FAIL:
                yield outcome

    # ----- lifecycle ------------------------------------------------------

    def __enter__(self) -> "Interface":
        self._started_at = time.monotonic()
        self._install_sigint()
        self._render_command_header()
        if not self.plain and not self.quiet:
            self._status = Status("preparing pipeline…", console=self.console,
                                    spinner="dots")
            self._status.__enter__()
        # Let credential prompts fired from deep inside backend
        # construction quiesce our live region while they draw — the
        # rich Status spinner and Progress deck both repaint on a
        # timer, which otherwise garbles the credential Panel/Prompt.
        _ba_config.register_prompt_suspend(self._suspend_for_prompt)
        return self

    def __exit__(self, exc_type, exc, tb) -> None:
        _ba_config.register_prompt_suspend(None)
        # Drop the spinner if it's still up (pipeline setup error or
        # zero-input run — no callbacks_for ever called).
        self._close_status()

        if not self._opened:
            self._open_run()

        # Drain non-terminal tasks. Two cases:
        #   - Interrupt (Ctrl-C): RUN tasks → fail "interrupted",
        #     WAIT tasks → skip "not started"
        #   - Other exception: stash it for the summary banner. RUN
        #     tasks → fail with the exception, WAIT tasks → skip
        #     "pipeline aborted" so they don't appear in the
        #     per-file fail list (the banner has the message).
        if self._interrupted:
            for t in self._tasks.values():
                if t.state is TaskState.RUN:
                    t.fail("interrupted")
                elif t.state is TaskState.WAIT:
                    t.skip("not started")
                else:
                    continue
                if self._progress is not None:
                    self._refresh_task(t)
        elif exc is not None:
            self._pipeline_error = f"{type(exc).__name__}: {exc}"
            for t in self._tasks.values():
                if t.state is TaskState.RUN:
                    t.fail(self._pipeline_error)
                elif t.state is TaskState.WAIT:
                    t.skip("pipeline aborted")
                else:
                    continue
                if self._progress is not None:
                    self._refresh_task(t)

        if self._progress is not None:
            self._progress.__exit__(exc_type, exc, tb)
            self._progress = None

        self._render_summary()
        self._restore_sigint()
        # Suppress any exception raised inside the `with` block: we've
        # already reported it via the summary ("pipeline aborted: …"
        # banner). The command then raises `typer.Exit(ui.exit_code)`
        # right after the `with`, so the exit status still reflects
        # the failure — no duplicate Rich traceback below the summary.
        return exc is not None

    # ----- exit code ------------------------------------------------------

    @property
    def exit_code(self) -> int:
        if self._interrupted:
            return 130
        if self._pipeline_error is not None:
            return 2
        states = [t.state for t in self._tasks.values()]
        if not states:
            return 2  # nothing ran — setup failure
        any_fail = any(s is TaskState.FAIL for s in states)
        any_ok   = any(s is TaskState.OK for s in states)
        if any_fail and any_ok:
            return 1
        if any_fail:
            return 2
        return 0

    # ----- header / summary rendering -------------------------------------

    def _open_run(self) -> None:
        """Close the spinner, print the file-count line, open the deck.

        Idempotent.
        """
        if self._opened:
            return
        self._opened = True
        self._close_status()
        self._render_file_count()
        if not self.plain and not self.quiet:
            self._open_progress()

    def _render_command_header(self) -> None:
        dest = "in-place" if self.output is None else f"→ {self.output}"
        line1 = f"batchalign3 {self.command} · {dest}"
        params_bits: list[str] = []
        for k, v in self.params.items():
            if v is None:
                continue
            if isinstance(v, bool):
                params_bits.append(f"{k} {'on' if v else 'off'}")
            else:
                params_bits.append(f"{k} {v}")
        line2 = " · ".join(params_bits) if params_bits else ""
        if self.plain:
            self.console.print(line1)
            if line2:
                self.console.print(line2)
        else:
            self.console.print(line1, style="bold")
            if line2:
                self.console.print(line2, style="dim")

    def _render_file_count(self) -> None:
        n = len(self._tasks)
        files_word = "file" if n == 1 else "files"
        if self.plain:
            self.console.print(f"{n} {files_word}")
        else:
            self.console.print(f"[dim]{n} {files_word}[/]")
        self.console.print()

    @contextmanager
    def _suspend_for_prompt(self):
        """Pause the active spinner/progress region for a credential prompt.

        Rich's Status and Progress widgets both repaint on a timer; if
        we leave them running while ``config._prompt_form`` draws its
        Panel and Prompt, the two redraw loops fight and we get the
        garbled output that the user reported (interleaved spinner
        glyphs and unreadable input line). Stopping the live region
        for the duration of the prompt — and restarting it after —
        keeps both surfaces legible.
        """
        status, self._status = self._status, None
        progress, self._progress = self._progress, None
        if status is not None:
            with suppress(Exception):
                status.__exit__(None, None, None)
        if progress is not None:
            with suppress(Exception):
                progress.stop()
        try:
            yield
        finally:
            if progress is not None:
                with suppress(Exception):
                    progress.start()
                self._progress = progress
            if status is not None:
                # Re-arm the same spinner so the user sees the original
                # "preparing pipeline…" status continue after they
                # finish entering credentials.
                new_status = Status(
                    "preparing pipeline…",
                    console=self.console,
                    spinner="dots",
                )
                with suppress(Exception):
                    new_status.__enter__()
                self._status = new_status

    def _close_status(self) -> None:
        if self._status is not None:
            self._status.__exit__(None, None, None)
            self._status = None

    def _render_summary(self) -> None:
        ok = sum(1 for t in self._tasks.values() if t.state is TaskState.OK)
        fail = sum(1 for t in self._tasks.values() if t.state is TaskState.FAIL)
        skip = sum(1 for t in self._tasks.values() if t.state is TaskState.SKIP)
        elapsed = time.monotonic() - self._started_at

        if self.plain:
            self.console.print()
            self.console.print(
                f"done   done={ok} fail={fail} skip={skip}  {_fmt_elapsed(elapsed)}"
            )
        else:
            self.console.print()
            self.console.print(
                f"done · [green]done {ok}[/] · [red]fail {fail}[/] · "
                f"[blue]skip {skip}[/] · {_fmt_elapsed(elapsed)}"
            )

        # Pipeline-level abort: one line with the exception, no
        # per-file fail list (would just repeat the same message).
        if self._pipeline_error is not None:
            self.console.print()
            rendered = render_error(self._pipeline_error)
            if is_rich(rendered):
                if self.plain:
                    self.console.print(
                        f"pipeline aborted: "
                        f"{normalise_one_line(self._pipeline_error)}"
                    )
                else:
                    self.console.print("[red]pipeline aborted[/]")
                    self.console.print(rendered)
            else:
                line = str(rendered)
                if self.plain:
                    self.console.print(f"pipeline aborted: {line}")
                else:
                    self.console.print(f"[red]pipeline aborted[/]: {line}")
            hint = hint_for(self._pipeline_error)
            if hint:
                indent = " " * (_W_STATE + 2)
                if self.plain:
                    self.console.print(f"{indent}hint: {hint}")
                else:
                    self.console.print(f"{indent}[dim]hint:[/] {hint}")
        else:
            # Per-file failure list — only when the pipeline ran to
            # completion and some sources failed individually. The
            # bars carry STATE; the summary carries the MESSAGE.
            failures = [t for t in self._tasks.values()
                        if t.state is TaskState.FAIL]
            if failures:
                self.console.print()
                for t in failures:
                    self._print_summary_failure(t)

        if self._interrupted:
            if self.plain:
                self.console.print("interrupted by user")
            else:
                self.console.print("[red]interrupted by user[/]")

    def _print_summary_failure(self, task: Task) -> None:
        msg = task.error or ""
        rendered = render_error(msg)
        # In the summary, alignment with the deck doesn't matter; using
        # the live-region padding here just produces a wall of spaces
        # between a short filename and its message. Print the label
        # tightly with two spaces of separation.
        if is_rich(rendered):
            # Parse error with caret block: header line is just the
            # failed file; the renderable below carries the detail.
            if self.plain:
                self.console.print(
                    f"fail  {task.label}  {normalise_one_line(msg)}"
                )
            else:
                self.console.print(f"[red]fail[/]  {task.label}")
                self.console.print(rendered)
        else:
            line = str(rendered)
            if self.plain:
                self.console.print(f"fail  {task.label}  {line}")
            else:
                self.console.print(f"[red]fail[/]  {task.label}  {line}")
        hint = hint_for(task.error)
        if hint:
            if self.plain:
                self.console.print(f"      hint: {hint}")
            else:
                self.console.print(f"      [dim]hint:[/] {hint}")
        self._print_verbose_traceback(task.error)

    def _print_verbose_traceback(self, error: str | None) -> None:
        if self.verbosity < 2:
            return
        traceback = extract_verbose_traceback(error)
        if not traceback:
            return
        if self.plain:
            self.console.print("      traceback:")
            for line in traceback.rstrip().splitlines():
                self.console.print(f"        {line}")
        else:
            self.console.print("      [dim]traceback:[/]")
            self.console.print(
                Syntax(
                    traceback,
                    "pytb",
                    word_wrap=False,
                    background_color="default",
                )
            )

    # ----- progress wiring ------------------------------------------------

    def _open_progress(self) -> None:
        single = len(self._tasks) == 1

        # Custom columns. Status column reads task.fields[state] +
        # task.fields[state_style]; markup=True (default) renders the
        # colour codes inline. Stage column similar.
        state_col = TextColumn(
            "[{task.fields[state_style]}]{task.fields[state]:<" + str(_W_STATE) + "}[/]"
        )
        label_col = TextColumn("{task.description}")
        stage_col = TextColumn(
            "[dim]{task.fields[stage]:<" + str(_W_STAGE) + "}[/]"
        )
        bar_col = BarColumn(bar_width=24)
        count_col = MofNCompleteColumn()
        elapsed_col = TimeElapsedColumn()
        eta_col = TimeRemainingColumn(elapsed_when_finished=False)

        if single:
            cols = [state_col, stage_col, bar_col, count_col, elapsed_col]
        else:
            cols = [state_col, label_col, stage_col, bar_col, count_col,
                    elapsed_col, eta_col]

        self._progress = Progress(
            *cols, console=self.console, transient=False, refresh_per_second=4,
        )
        self._progress.__enter__()

        if not single:
            # Overall bar — at the top, no state/stage column meaning.
            self._overall = self._progress.add_task(
                description="[bold]overall[/]",
                total=max(1, len(self._tasks)),
                state="",            # blank cell for the overall row
                state_style="dim",
                stage="",
            )
            # Pre-register every pushed file in WAIT state. All bars
            # visible from the start; the deck is the source of truth.
            for sid, task in self._tasks.items():
                rid = self._progress.add_task(
                    description=_pad(task.label, _W_LABEL),
                    total=1, completed=0,
                    state=task.state.value.lower(),
                    state_style=_STATE_STYLE[task.state],
                    stage="—",
                )
                self._rich_ids[sid] = rid
        # Single-file: stage bars added lazily on each StageStarted.

    def _on_event(self, ev: Any, task: Task) -> None:
        if not self._opened:
            self._open_run()
        if self.verbosity >= 2:
            _log.debug("progress %s: %s %s %s/%s",
                       task.source_id, ev.kind, getattr(ev, "task", None),
                       getattr(ev, "completed", 0), getattr(ev, "total", 0))

        if self.plain:
            self._plain_event(ev, task)
            return

        if self._progress is None:  # quiet mode
            return

        if len(self._tasks) == 1:
            self._refresh_single_file(task)
        else:
            self._refresh_task(task)
            if task.is_terminal:
                self._mark_completed(task)

    def _mark_completed(self, task: Task) -> None:
        """Credit `task` toward the overall counter exactly once.

        Called both from `_on_event` (terminal events from the Rust
        pipeline) and from `run_pipeline`'s per-input `except` block
        (terminal failures the Rust pipeline raises without firing a
        callback, e.g. parse errors during input conversion).
        """
        if task.source_id in self._counted_complete:
            return
        if self._progress is None or self._overall is None:
            return
        self._counted_complete.add(task.source_id)
        self._completed_count += 1
        self._progress.update(self._overall, completed=self._completed_count)

    # ----- batch refresh -------------------------------------------------

    def _refresh_task(self, task: Task) -> None:
        """Paint `task`'s current state onto its pre-registered bar."""
        assert self._progress is not None
        rid = self._rich_ids.get(task.source_id)
        if rid is None:
            return
        completed = task.progress[0] if task.progress else (
            1 if task.state is TaskState.OK else 0
        )
        total = task.progress[1] if task.progress else 1
        self._progress.update(
            rid,
            completed=completed,
            total=total,
            state=task.state.value.lower(),
            state_style=_STATE_STYLE[task.state],
            stage=(task.stage or "—"),
        )

    # ----- single-file refresh -------------------------------------------

    def _refresh_single_file(self, task: Task) -> None:
        assert self._progress is not None
        if task.stage is None and not task.is_terminal:
            return

        # When stage transitions, fill the previous stage bar to 100% +
        # mark it ok.
        if (
            self._single_prev_stage is not None
            and task.stage is not None
            and self._single_prev_stage != task.stage
        ):
            self._finish_stage_bar(self._single_prev_stage, TaskState.OK)
        if task.stage is not None:
            self._single_prev_stage = task.stage

        # Lazy-add the current stage bar.
        if task.stage is not None and task.stage not in self._stage_ids:
            rid = self._progress.add_task(
                description=_pad(task.stage, _W_STAGE),
                total=(task.progress[1] if task.progress else 1),
                completed=(task.progress[0] if task.progress else 0),
                state=task.state.value.lower(),
                state_style=_STATE_STYLE[task.state],
                stage=task.stage,
            )
            self._stage_ids[task.stage] = rid

        if task.stage is not None:
            rid = self._stage_ids[task.stage]
            completed = task.progress[0] if task.progress else 0
            total = task.progress[1] if task.progress else 1
            self._progress.update(
                rid,
                completed=completed,
                total=total,
                state=task.state.value.lower(),
                state_style=_STATE_STYLE[task.state],
                stage=task.stage,
            )

        if task.is_terminal and self._single_prev_stage is not None:
            self._finish_stage_bar(self._single_prev_stage, task.state)

    def _finish_stage_bar(self, stage: str, terminal_state: TaskState) -> None:
        if self._progress is None:
            return
        rid = self._stage_ids.get(stage)
        if rid is None:
            return
        rt = next((r for r in self._progress.tasks if r.id == rid), None)
        if rt is None:
            return
        self._progress.update(
            rid,
            completed=rt.total or 1,
            state=terminal_state.value.lower(),
            state_style=_STATE_STYLE[terminal_state],
        )

    # ----- plain renderer -------------------------------------------------

    def _plain_event(self, ev: Any, task: Task) -> None:
        from batchalign._core import ProgressKind  # type: ignore[attr-defined]
        # First `start` line per file.
        if (
            ev.kind is ProgressKind.StageStarted
            and task.stage
            and task.source_id not in self._plain_started
        ):
            self._plain_started.add(task.source_id)
            self.console.print(f"start  {_pad(task.label, _W_LABEL)}  {task.stage}")
        # One terminal line per file.
        if task.is_terminal and task.source_id not in self._plain_completed:
            self._plain_completed.add(task.source_id)
            state = task.state.value.lower()
            time_str = (_fmt_elapsed(task.elapsed) if task.elapsed is not None
                        else "")
            self.console.print(
                f"{state:<{_W_STATE}}   {_pad(task.label, _W_LABEL)}  "
                f"{time_str:>{_W_TIME}}"
            )

    # ----- SIGINT ---------------------------------------------------------

    def _install_sigint(self) -> None:
        # Two-stage Ctrl-C, because the Rust pipeline holds the main
        # thread inside `py.detach` for the duration of a run:
        #
        #   1st Ctrl-C: mark interrupted, call `pipeline.cancel()` so the
        #               engine stops dispatching new work, then raise
        #               KeyboardInterrupt. The raise only takes effect
        #               once `pipeline.run` returns (Rust polls signals
        #               itself — see `pipeline.rs` — so that happens
        #               within ~100 ms after the cancel flag is set + the
        #               currently-running backend calls finish).
        #
        #   2nd Ctrl-C within 2 s: hard exit. `os._exit(130)` skips
        #               atexit + Python finalization, but the kernel
        #               sends SIGTERM/HUP to subprocesses in the
        #               foreground process group, so ffmpeg / whisper /
        #               stanza children die with us instead of being
        #               orphaned. This is the escape hatch for when a
        #               backend has gone unresponsive and won't honor
        #               cooperative cancel.
        def _handler(signum, frame):  # noqa: ARG001
            now = time.monotonic()
            if self._interrupted and (now - self._interrupt_at) < 2.0:
                # Bypass the live region's atexit ordering — we want to
                # be gone *now*, not after Rich tries to redraw.
                os._exit(130)
            self._interrupted = True
            self._interrupt_at = now
            if self._pipeline is not None:
                with suppress(Exception):
                    self._pipeline.cancel()
            raise KeyboardInterrupt
        try:
            self._sigint_prev = signal.signal(signal.SIGINT, _handler)
        except (ValueError, OSError):
            self._sigint_prev = None

    def _restore_sigint(self) -> None:
        if self._sigint_prev is not None:
            with suppress(ValueError, OSError):
                signal.signal(signal.SIGINT, self._sigint_prev)
            self._sigint_prev = None


# --------------------------------------------------------------------------
# Small helpers — formatting only.
# --------------------------------------------------------------------------

def _fmt_elapsed(secs: float | None) -> str:
    if secs is None:
        return ""
    if secs < 60:
        return f"{secs:>4.1f}s"
    m, s = divmod(int(secs), 60)
    return f"{m:02d}:{s:02d}"


def _pad(text: str, width: int) -> str:
    """Pad/truncate `text` to exactly `width` chars (head+tail trim)."""
    if len(text) <= width:
        return text.ljust(width)
    keep = width - 1
    head = keep // 2
    tail = keep - head
    return text[:head] + "…" + text[-tail:]


__all__ = ["Interface"]
