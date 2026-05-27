"""Task state-machine invariants."""

from __future__ import annotations

import pytest

from batchalign.cli.tui.task import Task, TaskState, TaskStateError


def _t(label: str = "x.wav") -> Task:
    return Task(source_id="x", label=label)


def test_starts_in_wait():
    t = _t()
    assert t.state is TaskState.WAIT
    assert t.started is None
    assert t.elapsed is None


def test_start_advances_to_run():
    t = _t()
    t.start()
    assert t.state is TaskState.RUN
    assert t.started is not None
    assert t.elapsed is not None


def test_start_is_idempotent():
    t = _t()
    t.start()
    started_first = t.started
    t.start()
    assert t.started == started_first


def test_stage_started_auto_starts():
    t = _t()
    t.stage_started("asr")
    assert t.state is TaskState.RUN
    assert t.stage == "asr"


def test_stage_started_resets_progress():
    t = _t()
    t.start()
    t.stage_started("asr")
    t.update(5, 10)
    assert t.progress == (5, 10)
    t.stage_started("fa")
    assert t.progress is None
    assert t.stage == "fa"


def test_update_ignored_with_zero_total():
    t = _t()
    t.start()
    t.update(0, 0)
    assert t.progress is None


def test_update_ignored_when_not_running():
    t = _t()
    t.update(1, 2)
    assert t.progress is None


def test_complete_terminal():
    t = _t()
    t.start()
    t.complete()
    assert t.state is TaskState.OK
    assert t.is_terminal
    assert t.finished is not None


def test_complete_idempotent():
    t = _t()
    t.start()
    t.complete()
    finished_first = t.finished
    t.complete()
    assert t.finished == finished_first
    assert t.state is TaskState.OK


def test_fail_terminal_with_message():
    t = _t()
    t.start()
    t.fail("cuda OOM")
    assert t.state is TaskState.FAIL
    assert t.error == "cuda OOM"


def test_fail_empty_message_normalised():
    t = _t()
    t.start()
    t.fail("")
    assert t.error == "<no message>"


def test_skip_terminal():
    t = _t()
    t.skip("no audio")
    assert t.state is TaskState.SKIP
    assert t.error == "no audio"


def test_terminal_state_resists_re_complete():
    t = _t()
    t.start()
    t.fail("boom")
    t.complete()  # no-op
    assert t.state is TaskState.FAIL


def test_illegal_start_from_terminal_raises():
    t = _t()
    t.start()
    t.complete()
    with pytest.raises(TaskStateError):
        t.start()


def test_stage_started_from_terminal_raises():
    t = _t()
    t.start()
    t.complete()
    with pytest.raises(TaskStateError):
        t.stage_started("morph")


def test_from_input_uses_basename():
    class StubInput:
        source_id = "/abs/path/01_intake.wav"
        path = "/abs/path/01_intake.wav"

    t = Task.from_input(StubInput())
    assert t.label == "01_intake.wav"
    assert t.source_id == "/abs/path/01_intake.wav"
