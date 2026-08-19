"""Interaction and responsive-layout coverage for the Textual dashboard."""

from __future__ import annotations

import asyncio
import io
import threading

from rich.console import Console
from textual.widgets import DataTable, Static

from batchalign.cli.tui.dashboard import BatchalignDashboard, TaskSnapshot
from batchalign.cli.tui.task import Task, TaskState


def _snapshots() -> list[TaskSnapshot]:
    return [
        TaskSnapshot("/data/a.wav", "a.wav", TaskState.RUN, "asr", 2, 10, 1.2, None),
        TaskSnapshot(
            "/data/b.wav",
            "b.wav",
            TaskState.FAIL,
            "morph",
            None,
            None,
            2.5,
            "whisper: cuda OOM",
        ),
        TaskSnapshot(
            "/data/c.wav", "c.wav", TaskState.WAIT, None, None, None, None, None
        ),
    ]


def _render_text(renderable) -> str:
    output = io.StringIO()
    Console(file=output, force_terminal=False, no_color=True, width=100).print(
        renderable
    )
    return output.getvalue()


def test_task_snapshot_detaches_mutable_task_state():
    task = Task(source_id="a", label="a.wav")
    task.stage_started("asr")
    task.update(3, 8)
    snapshot = TaskSnapshot.from_task(task)

    task.update(7, 8)
    task.complete()

    assert snapshot.state is TaskState.RUN
    assert (snapshot.completed, snapshot.total) == (3, 8)


def test_pipeline_is_released_only_after_initial_refresh():
    async def exercise() -> None:
        ready = threading.Event()
        app = BatchalignDashboard(
            command="transcribe",
            params={},
            output=None,
            snapshots=_snapshots(),
            ready=ready,
        )
        assert not ready.is_set()
        async with app.run_test(size=(100, 28)) as pilot:
            await pilot.pause()
            assert ready.is_set()
            assert app._content_ready
            assert app.query_one("#files", DataTable).row_count == 3
            app.exit()

    asyncio.run(exercise())


def test_elapsed_clock_advances_without_progress_events():
    async def exercise() -> None:
        app = BatchalignDashboard(
            command="convert",
            params={"format": "mp3"},
            output=None,
            snapshots=_snapshots(),
        )
        async with app.run_test(size=(100, 28)) as pilot:
            await pilot.pause()
            running_before = app.snapshots[0].elapsed
            terminal_before = app.snapshots[1].elapsed

            # Simulate a quiet two-second encode stage and trigger the same
            # callback the dashboard's 100 ms interval invokes.
            app._last_elapsed_refresh -= 2.0
            app._refresh_elapsed()

            assert app.snapshots[0].elapsed is not None
            assert running_before is not None
            assert app.snapshots[0].elapsed >= running_before + 2.0
            assert app.snapshots[1].elapsed == terminal_before
            table = app.query_one("#files", DataTable)
            assert table.get_cell("/data/a.wav", "elapsed") == (
                f"{app.snapshots[0].elapsed:.1f}s"
            )
            app.exit()

    asyncio.run(exercise())


def test_dashboard_filters_navigates_and_responds_to_resize():
    async def exercise() -> None:
        app = BatchalignDashboard(
            command="transcribe",
            params={"engine": "whisper", "lang": "eng"},
            output=None,
            snapshots=_snapshots(),
        )
        async with app.run_test(size=(120, 36)) as pilot:
            await pilot.pause()
            table = app.query_one("#files", DataTable)
            assert table.row_count == 3
            assert not app.screen.has_class("narrow")
            assert "whisper" in str(app.query_one("#config", Static).render())
            assert "Ctrl+C" in str(app.query_one("#keybar", Static).render())
            filters = str(app.query_one("#filters", Static).render())
            assert "ALL 3" in filters
            assert "ACTIVE 2" in filters
            assert "1 ALL" not in filters

            await pilot.press("down")
            await pilot.pause()
            assert app.selected_source_id == "/data/b.wav"
            assert "cuda OOM" in str(app.query_one("#detail-body", Static).render())

            await pilot.press("4")
            await pilot.pause()
            assert app.filter_name == "failed"
            assert table.row_count == 1
            assert app.selected_source_id == "/data/b.wav"

            await pilot.resize_terminal(70, 22)
            await pilot.pause()
            assert app.screen.has_class("narrow")
            assert not app.query_one("#detail-panel").display

            await pilot.press("d")
            await pilot.pause()
            assert app.query_one("#detail-panel").display

            await pilot.press("d")
            await pilot.pause()
            assert not app.query_one("#detail-panel").display

            await pilot.resize_terminal(60, 20)
            await pilot.pause()
            assert app.screen.has_class("compact")
            assert app.screen.has_class("too-small")
            warning = app.query_one("#size-warning", Static)
            assert warning.display
            assert "enlarge the window" in str(warning.render())
            assert "^C" in str(app.query_one("#keybar", Static).render())

            await pilot.press("d")
            await pilot.pause()
            await pilot.resize_terminal(120, 36)
            await pilot.pause()
            assert not app.screen.has_class("narrow")
            assert not app.screen.has_class("too-small")
            assert not app.query_one("#size-warning", Static).display
            assert app.query_one("#detail-panel").display
            app.exit()

    asyncio.run(exercise())


def test_dashboard_applies_live_updates_and_preserves_selection():
    async def exercise() -> None:
        app = BatchalignDashboard(
            command="morphotag", params={}, output=None, snapshots=_snapshots()
        )
        async with app.run_test(size=(110, 30)) as pilot:
            await pilot.pause()
            await pilot.press("down")
            await pilot.pause()
            selected = app.selected_source_id

            updated = list(_snapshots())
            updated[0] = TaskSnapshot(
                "/data/a.wav", "a.wav", TaskState.OK, "asr", None, None, 3.0, None
            )
            app.apply_snapshots(updated)
            await pilot.pause()

            assert app.selected_source_id == selected
            summary = str(app.query_one("#summary", Static).render())
            assert "1 done" in summary
            app.exit()

    asyncio.run(exercise())


def test_live_progress_does_not_recenter_the_users_viewport():
    async def exercise() -> None:
        snapshots = [
            TaskSnapshot(
                f"/data/{index:02}.wav",
                f"{index:02}.wav",
                TaskState.RUN if index == 0 else TaskState.WAIT,
                "asr" if index == 0 else None,
                index if index == 0 else None,
                100 if index == 0 else None,
                1.0 if index == 0 else None,
                None,
            )
            for index in range(30)
        ]
        app = BatchalignDashboard(
            command="transcribe", params={}, output=None, snapshots=snapshots
        )
        async with app.run_test(size=(100, 20)) as pilot:
            await pilot.pause()
            table = app.query_one("#files", DataTable)
            assert app.selected_source_id == "/data/00.wav"
            table.scroll_to(y=12, animate=False, immediate=True)
            await pilot.pause()
            before = table.scroll_offset
            assert before.y > 0

            updated = list(snapshots)
            updated[0] = TaskSnapshot(
                "/data/00.wav",
                "00.wav",
                TaskState.RUN,
                "asr",
                50,
                100,
                2.0,
                None,
            )
            app.apply_snapshots(updated)
            await pilot.pause()

            assert table.scroll_offset == before
            assert app.selected_source_id == "/data/00.wav"
            assert str(table.get_cell("/data/00.wav", "progress")) == "50/100   50%"
            app.exit()

    asyncio.run(exercise())


def test_dashboard_surfaces_cancellation_immediately():
    async def exercise() -> None:
        cancellations = []
        app = BatchalignDashboard(
            command="align",
            params={},
            output=None,
            snapshots=_snapshots(),
            request_cancel=lambda: cancellations.append(True),
        )
        async with app.run_test(size=(100, 28)) as pilot:
            await pilot.pause()
            await pilot.press("ctrl+c")
            await pilot.pause()

            assert cancellations == [True]
            assert app._cancel_requested
            summary = str(app.query_one("#summary", Static).render())
            assert "CANCELLING" in summary
            assert "again" in str(app.query_one("#keybar", Static).render())

            await pilot.press("ctrl+c")
            await pilot.pause()
            assert cancellations == [True, True]
            app.exit()

    asyncio.run(exercise())


def test_dashboard_preserves_structured_chat_parse_rendering(tmp_path):
    async def exercise() -> None:
        chat = tmp_path / "broken.cha"
        content = "@Begin\n*PAR:\thello <bad>\n@End\n"
        chat.write_text(content, encoding="utf-8")
        start = content.encode().index(b"<bad>")
        error = (
            f"parse {chat}: CHAT validation failed: "
            f"error[E999]: Illegal token (bytes {start}..{start + 5})"
        )
        app = BatchalignDashboard(
            command="morphotag",
            params={},
            output=None,
            snapshots=[
                TaskSnapshot(
                    str(chat),
                    chat.name,
                    TaskState.FAIL,
                    "morphosyntax",
                    None,
                    None,
                    0.5,
                    error,
                )
            ],
        )
        async with app.run_test(size=(120, 32)) as pilot:
            await pilot.pause()
            rendered = _render_text(app.query_one("#detail-body", Static).content)
            assert "error[E999]: Illegal token" in rendered
            assert "at line 2" in rendered
            assert "hello <bad>" in rendered
            assert "^^^^^" in rendered
            app.exit()

    asyncio.run(exercise())
