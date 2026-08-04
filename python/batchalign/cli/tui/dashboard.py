"""Responsive Textual dashboard used by the interactive CLI.

The processing engine is synchronous and may call progress callbacks from
worker threads.  It therefore never touches Textual widgets directly:
``Dashboard`` turns mutable :class:`Task` objects into immutable snapshots and
posts those snapshots to the Textual event loop.
"""

from __future__ import annotations

import threading
from collections.abc import Callable, Iterable, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import ClassVar

from rich.console import Group
from rich.markup import escape
from rich.text import Text
from textual import events
from textual.app import App, ComposeResult
from textual.binding import Binding
from textual.containers import Horizontal, Vertical
from textual.widgets import DataTable, ProgressBar, Static

from .errors import is_rich, render_error
from .hints import hint_for
from .task import Task, TaskState


@dataclass(frozen=True)
class TaskSnapshot:
    """The display-safe, thread-safe projection of one pipeline task."""

    source_id: str
    label: str
    state: TaskState
    stage: str | None
    completed: int | None
    total: int | None
    elapsed: float | None
    error: str | None

    @classmethod
    def from_task(cls, task: Task) -> TaskSnapshot:
        completed, total = task.progress or (None, None)
        return cls(
            source_id=task.source_id,
            label=task.label,
            state=task.state,
            stage=task.stage,
            completed=completed,
            total=total,
            elapsed=task.elapsed,
            error=task.error,
        )


_STATE_MARKUP = {
    TaskState.WAIT: "[dim]○ QUEUED[/]",
    TaskState.RUN: "[bold #7dd3fc]● RUNNING[/]",
    TaskState.OK: "[bold #6ee7b7]✓ DONE[/]",
    TaskState.FAIL: "[bold #fb7185]✗ FAILED[/]",
    TaskState.SKIP: "[bold #fbbf24]– SKIPPED[/]",
}
_FILTERS: tuple[tuple[str, frozenset[TaskState]], ...] = (
    ("all", frozenset(TaskState)),
    ("active", frozenset((TaskState.WAIT, TaskState.RUN))),
    ("done", frozenset((TaskState.OK,))),
    ("failed", frozenset((TaskState.FAIL,))),
    ("skipped", frozenset((TaskState.SKIP,))),
)


def _elapsed(seconds: float | None) -> str:
    if seconds is None:
        return "—"
    if seconds < 60:
        return f"{seconds:.1f}s"
    minutes, secs = divmod(int(seconds), 60)
    return f"{minutes:02d}:{secs:02d}"


def _progress(task: TaskSnapshot) -> str:
    if task.completed is not None and task.total:
        percent = min(100, round(100 * task.completed / task.total))
        return f"{task.completed}/{task.total}  {percent:>3}%"
    if task.state is TaskState.OK:
        return "100%"
    if task.state is TaskState.RUN:
        return "working…"
    return "—"


class BatchalignDashboard(App[None]):
    """Full-screen, keyboard-navigable view of one local processing run."""

    TITLE = "batchalign"
    ENABLE_COMMAND_PALETTE = False
    BINDINGS: ClassVar = [
        Binding("up,k", "cursor_up", "Previous", show=False),
        Binding("down,j", "cursor_down", "Next", show=False),
        Binding("home,g", "first", "First", show=False),
        Binding("end,shift+g", "last", "Last", show=False),
        Binding("f", "cycle_filter", "Filter"),
        Binding("1", "set_filter('all')", "All", show=False),
        Binding("2", "set_filter('active')", "Active", show=False),
        Binding("3", "set_filter('done')", "Done", show=False),
        Binding("4", "set_filter('failed')", "Failed", show=False),
        Binding("5", "set_filter('skipped')", "Skipped", show=False),
        Binding("d,enter", "toggle_detail", "Details"),
        Binding("q", "dismiss", "Hide"),
        Binding("ctrl+c", "cancel_run", "Cancel"),
    ]

    CSS = """
    Screen {
        background: #090d16;
        color: #d7deea;
    }
    #masthead {
        height: 3;
        padding: 0 2;
        background: #111827;
        border-bottom: solid #26334a;
        content-align: left middle;
    }
    #masthead > Static { height: 1; content-align: left middle; }
    #brand { width: 16; color: #a5b4fc; text-style: bold; }
    #command { width: auto; color: #f8fafc; text-style: bold; }
    #destination { width: 1fr; padding-left: 2; color: #8492a6; }
    #overview {
        height: 7;
        padding: 1 2 0 2;
        background: #0d1320;
    }
    #summary { height: 1; }
    #overall { height: 1; margin-top: 1; }
    ProgressBar > Bar { color: #818cf8; background: #202b3d; }
    ProgressBar > PercentageStatus { color: #a5b4fc; width: 6; }
    #filters { height: 2; color: #91a0b8; padding-top: 1; }
    #config { height: 1; color: #64748b; }
    #size-warning {
        display: none;
        height: 1;
        padding: 0 1;
        background: #422006;
        color: #fde68a;
        text-style: bold;
        content-align: center middle;
    }
    Screen.too-small #size-warning { display: block; }
    #workspace { height: 1fr; padding: 0 1 1 1; }
    #files-panel {
        width: 2fr;
        border: round #26334a;
        background: #0c111c;
    }
    #files-title {
        height: 2;
        padding: 0 1;
        color: #a5b4fc;
        text-style: bold;
        content-align: left middle;
    }
    DataTable { height: 1fr; background: #0c111c; }
    DataTable > .datatable--header {
        background: #141d2c;
        color: #91a0b8;
        text-style: bold;
    }
    DataTable > .datatable--cursor { background: #202c43; color: #ffffff; }
    #detail-panel {
        width: 1fr;
        min-width: 30;
        margin-left: 1;
        padding: 1 2;
        border: round #334155;
        background: #101725;
        overflow-y: auto;
    }
    #detail-title { color: #a5b4fc; text-style: bold; margin-bottom: 1; }
    #detail-state { margin-bottom: 1; }
    #detail-body { color: #aeb9ca; }
    #narrow-note {
        display: none;
        height: 1;
        color: #64748b;
        padding-left: 2;
    }
    Screen.narrow #detail-panel { display: none; }
    Screen.narrow #files-panel { width: 1fr; }
    Screen.narrow #narrow-note { display: block; }
    Screen.compact #destination { display: none; }
    Screen.compact #config { display: none; }
    Screen.compact #workspace { padding-left: 0; padding-right: 0; }
    #keybar {
        height: 1;
        padding: 0 2;
        background: #111827;
        color: #91a0b8;
    }
    """

    def __init__(
        self,
        *,
        command: str,
        params: dict[str, object],
        output: Path | None,
        snapshots: Sequence[TaskSnapshot],
        ready: threading.Event | None = None,
        request_cancel: Callable[[], None] | None = None,
    ) -> None:
        super().__init__()
        self.command = command
        self.params = params
        self.output = output
        self.snapshots = list(snapshots)
        self.filter_name = "all"
        self.detail_visible = True
        self._external_ready_event = ready
        self._request_cancel = request_cancel
        self._finished = False
        self._cancel_requested = False
        self._content_ready = False

    def compose(self) -> ComposeResult:
        destination = "in place" if self.output is None else str(self.output)
        with Horizontal(id="masthead"):
            yield Static("BATCHALIGN", id="brand")
            yield Static(f"[#475569]│[/]  {escape(self.command)}", id="command")
            yield Static(f"→ {escape(destination)}", id="destination")
        with Vertical(id="overview"):
            yield Static(id="summary")
            yield ProgressBar(
                total=max(1, len(self.snapshots)), show_eta=False, id="overall"
            )
            yield Static(id="filters")
            yield Static(id="config")
        yield Static(
            "Terminal too small — enlarge the window for full details",
            id="size-warning",
        )
        with Horizontal(id="workspace"):
            with Vertical(id="files-panel"):
                yield Static("FILES", id="files-title")
                yield DataTable(cursor_type="row", zebra_stripes=True, id="files")
            with Vertical(id="detail-panel"):
                yield Static("FILE DETAILS", id="detail-title")
                yield Static(id="detail-state")
                yield Static(id="detail-body")
        yield Static("Press d for details", id="narrow-note")
        yield Static(
            "[bold #e0e7ff]↑↓/j k[/] navigate  [#475569]│[/]  "
            "[bold #e0e7ff]1–5/f[/] filter  [#475569]│[/]  "
            "[bold #e0e7ff]d/Enter[/] details  [#475569]│[/]  "
            "[bold #fbbf24]Ctrl+C[/] cancel",
            id="keybar",
        )

    def on_mount(self) -> None:
        table = self.query_one("#files", DataTable)
        table.add_columns("STATUS", "FILE", "STAGE", "PROGRESS", "ELAPSED")
        table.focus()
        self._content_ready = True
        self._apply_size_classes(self.size.width, self.size.height)
        self._render_all()
        if self._external_ready_event is not None:
            self._external_ready_event.set()

    def on_resize(self, event: events.Resize) -> None:
        self._apply_size_classes(event.size.width, event.size.height)
        if self._content_ready:
            self._render_all(self.selected_source_id)

    def _apply_size_classes(self, width: int, height: int) -> None:
        self.screen.set_class(width < 96, "narrow")
        self.screen.set_class(width < 68, "compact")
        self.screen.set_class(width < 68 or height < 20, "too-small")
        if self._content_ready:
            self.query_one("#keybar", Static).update(
                "[bold #e0e7ff]↑↓[/] nav [#475569]·[/] "
                "[bold #e0e7ff]f[/] filter [#475569]·[/] "
                "[bold #e0e7ff]d[/] detail [#475569]·[/] "
                "[bold #fbbf24]^C[/] cancel"
                if width < 68
                else "[bold #e0e7ff]↑↓/j k[/] navigate  [#475569]│[/]  "
                "[bold #e0e7ff]1–5/f[/] filter  [#475569]│[/]  "
                "[bold #e0e7ff]d/Enter[/] details  [#475569]│[/]  "
                "[bold #fbbf24]Ctrl+C[/] cancel"
            )

    def apply_snapshots(
        self, snapshots: Sequence[TaskSnapshot], finished: bool = False
    ) -> None:
        """Apply an update inside the Textual event loop."""
        selected = self.selected_source_id
        self.snapshots = list(snapshots)
        self._finished = finished
        self._render_all(selected)
        if finished:
            self.set_timer(1.25, self.exit)

    @property
    def selected_source_id(self) -> str | None:
        table = self.query_one("#files", DataTable)
        if table.row_count == 0 or table.cursor_row < 0:
            return None
        try:
            return str(
                table.coordinate_to_cell_key(table.cursor_coordinate).row_key.value
            )
        except Exception:  # noqa: BLE001 - table may be mid-repopulation
            return None

    def _visible(self) -> list[TaskSnapshot]:
        states = dict(_FILTERS)[self.filter_name]
        return [task for task in self.snapshots if task.state in states]

    def _render_all(self, selected: str | None = None) -> None:
        counts = {
            state: sum(t.state is state for t in self.snapshots) for state in TaskState
        }
        terminal = (
            counts[TaskState.OK] + counts[TaskState.FAIL] + counts[TaskState.SKIP]
        )
        total = len(self.snapshots)
        run_state = "[bold #fbbf24]CANCELLING…[/]    " if self._cancel_requested else ""
        if self.screen.has_class("narrow"):
            summary = (
                f"{run_state}[bold]{terminal}/{total} complete[/]   "
                f"[#6ee7b7]✓{counts[TaskState.OK]}[/]  "
                f"[#7dd3fc]●{counts[TaskState.RUN]}[/]  "
                f"[#94a3b8]○{counts[TaskState.WAIT]}[/]  "
                f"[#fb7185]✗{counts[TaskState.FAIL]}[/]  "
                f"[#fbbf24]–{counts[TaskState.SKIP]}[/]"
            )
        else:
            summary = (
                f"{run_state}[bold]{terminal}/{total} complete[/]    "
                f"[#6ee7b7]✓ {counts[TaskState.OK]} done[/]    "
                f"[#7dd3fc]● {counts[TaskState.RUN]} running[/]    "
                f"[#94a3b8]○ {counts[TaskState.WAIT]} queued[/]    "
                f"[#fb7185]✗ {counts[TaskState.FAIL]} failed[/]    "
                f"[#fbbf24]– {counts[TaskState.SKIP]} skipped[/]"
            )
        self.query_one("#summary", Static).update(summary)
        self.query_one("#overall", ProgressBar).update(
            total=max(1, total), progress=terminal
        )
        filter_bits = []
        for index, (name, states) in enumerate(_FILTERS, 1):
            count = sum(t.state in states for t in self.snapshots)
            label = f"{index} {name.upper()} {count}"
            filter_bits.append(
                f"[bold #c7d2fe on #26334a] {label} [/]"
                if name == self.filter_name
                else f"[#738199] {label} [/]"
            )
        filters = "  ".join(filter_bits)
        if self.screen.has_class("narrow"):
            filters = filters.replace("ACTIVE", "ACT").replace("SKIPPED", "SKIP")
        self.query_one("#filters", Static).update(filters)
        config = "  ·  ".join(
            f"{escape(str(key))} [#aeb9ca]{escape(str(value))}[/]"
            for key, value in self.params.items()
            if value is not None
        )
        self.query_one("#config", Static).update(config or "default configuration")
        self._render_table(selected)
        self._render_detail()

    def _render_table(self, selected: str | None = None) -> None:
        table = self.query_one("#files", DataTable)
        table.clear(columns=False)
        visible = self._visible()
        for task in visible:
            table.add_row(
                _STATE_MARKUP[task.state],
                escape(task.label),
                escape(task.stage or "—"),
                _progress(task),
                _elapsed(task.elapsed),
                key=task.source_id,
            )
        if selected is not None:
            for index, task in enumerate(visible):
                if task.source_id == selected:
                    table.move_cursor(row=index)
                    break

    def _selected(self) -> TaskSnapshot | None:
        source_id = self.selected_source_id
        return next((t for t in self.snapshots if t.source_id == source_id), None)

    def _render_detail(self) -> None:
        task = self._selected()
        state = self.query_one("#detail-state", Static)
        body = self.query_one("#detail-body", Static)
        if task is None:
            state.update("[#64748b]No files in this view[/]")
            body.update("Change the filter with [bold]f[/] or number keys 1–5.")
            return
        state.update(_STATE_MARKUP[task.state])
        lines = [
            f"[bold #f8fafc]{escape(task.label)}[/]",
            f"[#64748b]{escape(task.source_id)}[/]",
            "",
            f"Stage     [#c7d2fe]{escape(task.stage or '—')}[/]",
            f"Progress  {_progress(task)}",
            f"Elapsed   {_elapsed(task.elapsed)}",
        ]
        if task.error:
            rendered = render_error(task.error)
            hint = hint_for(task.error)
            if is_rich(rendered):
                parts = [
                    Text.from_markup("\n".join(lines)),
                    Text(""),
                    Text("ERROR", style="bold #fb7185"),
                    rendered,
                ]
                if hint:
                    parts.extend(
                        (
                            Text(""),
                            Text.assemble(("Hint: ", "#fbbf24"), hint),
                        )
                    )
                body.update(Group(*parts))
                return
            lines.extend(("", "[bold #fb7185]ERROR[/]", escape(str(rendered))))
            if hint:
                lines.extend(("", f"[#fbbf24]Hint:[/] {escape(hint)}"))
        body.update("\n".join(lines))

    def on_data_table_row_highlighted(self, _event: DataTable.RowHighlighted) -> None:
        self._render_detail()

    def action_cursor_up(self) -> None:
        self.query_one("#files", DataTable).action_cursor_up()

    def action_cursor_down(self) -> None:
        self.query_one("#files", DataTable).action_cursor_down()

    def action_first(self) -> None:
        self.query_one("#files", DataTable).move_cursor(row=0)

    def action_last(self) -> None:
        table = self.query_one("#files", DataTable)
        table.move_cursor(row=max(0, table.row_count - 1))

    def action_cycle_filter(self) -> None:
        names = [name for name, _ in _FILTERS]
        self.filter_name = names[(names.index(self.filter_name) + 1) % len(names)]
        self._render_all()

    def action_set_filter(self, name: str) -> None:
        if name in dict(_FILTERS):
            self.filter_name = name
            self._render_all()

    def action_toggle_detail(self) -> None:
        panel = self.query_one("#detail-panel")
        self.detail_visible = not panel.display
        panel.display = self.detail_visible

    def action_dismiss(self) -> None:
        if self._finished:
            self.exit()
        else:
            self.notify(
                "Processing is still running; Ctrl+C cancels safely.",
                severity="warning",
            )

    def action_cancel_run(self) -> None:
        if self._request_cancel is not None:
            if self._cancel_requested:
                self.notify("Cancellation is already in progress.")
                return
            self._cancel_requested = True
            self._render_all(self.selected_source_id)
            self.notify(
                "Cancellation requested; finishing active work.", severity="warning"
            )
            self._request_cancel()


class Dashboard:
    """Lifecycle adapter that keeps Textual on the process main thread."""

    def __init__(
        self,
        *,
        command: str,
        params: dict[str, object],
        output: Path | None,
        tasks: Iterable[Task],
        request_cancel: Callable[[], None] | None = None,
    ) -> None:
        self._ready = threading.Event()
        self._app = BatchalignDashboard(
            command=command,
            params=params,
            output=output,
            snapshots=[TaskSnapshot.from_task(task) for task in tasks],
            ready=self._ready,
            request_cancel=request_cancel,
        )
        self._running = False

    def start(self) -> None:
        """Retained as a no-op for ``Interface``'s prepare/run boundary.

        Textual cannot install resize/job-control signal handlers from a
        background thread.  ``run_while`` starts the actual UI once the
        pipeline callable is ready.
        """

    def update(self, tasks: Iterable[Task], *, finished: bool = False) -> None:
        snapshots = [TaskSnapshot.from_task(task) for task in tasks]
        if not self._running or not self._ready.is_set():
            return
        try:
            self._app.call_from_thread(self._app.apply_snapshots, snapshots, finished)
        except RuntimeError:
            pass

    def run_while(
        self,
        worker: Callable[[], None],
        tasks: Iterable[Task],
        *,
        on_error: Callable[[BaseException], None] | None = None,
    ) -> None:
        """Run the dashboard while ``worker`` processes files off-thread."""
        errors: list[BaseException] = []

        def work() -> None:
            self._ready.wait(timeout=2.0)
            try:
                worker()
            except BaseException as exc:  # noqa: BLE001 - re-raised on main thread
                errors.append(exc)
                if on_error is not None:
                    on_error(exc)
            finally:
                self.update(tasks, finished=True)

        pipeline_thread = threading.Thread(
            target=work,
            name="batchalign-pipeline",
            daemon=False,
        )
        self._running = True
        pipeline_thread.start()
        try:
            self._app.run(mouse=True)
        finally:
            pipeline_thread.join()
            self._running = False
        if errors:
            raise errors[0]

    def close(self, tasks: Iterable[Task]) -> None:
        # Normal interactive runs are already closed by ``run_while``.
        # This only matters for setup failures before the pipeline starts.
        if self._running:
            self.update(tasks, finished=True)


__all__ = ["BatchalignDashboard", "Dashboard", "TaskSnapshot"]
