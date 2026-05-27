"""Shared pytest fixtures for batchalign Python tests.

Most tests should run without the compiled `_core` extension. Where a
test depends on the .so, it should `pytest.importorskip("batchalign._core")`.
"""

from __future__ import annotations

import sys
from pathlib import Path

import pytest


# Make `python/` importable when pytest is invoked from a different cwd
# (e.g. from the repo root via `pytest python/batchalign/tests`).
_PYTHON_ROOT = Path(__file__).resolve().parents[2]
if str(_PYTHON_ROOT) not in sys.path:
    sys.path.insert(0, str(_PYTHON_ROOT))


@pytest.fixture
def core_available() -> bool:
    """Whether the compiled `batchalign._core` extension is importable.

    Tests that require it should `pytest.skip()` when this is False.
    """
    try:
        import batchalign._core  # noqa: F401
        return True
    except ImportError:
        return False


@pytest.fixture
def tmp_ini(tmp_path: Path) -> Path:
    """Yield a temporary `.batchalign.ini` path; tests write into it."""
    return tmp_path / ".batchalign.ini"


@pytest.fixture
def fake_progress_core(monkeypatch):
    """Install a minimal fake `batchalign._core` exposing the names the
    TUI bridge and `Interface._plain_event` lazily import.

    Returns the (Task, ProgressKind, ProgressEvent) trio so tests can
    construct stub events.
    """
    import enum
    import types

    class Task(str, enum.Enum):
        Asr = "Asr"
        Fa = "Fa"
        Speaker = "Speaker"
        UtSeg = "UtSeg"
        Morphosyntax = "Morphosyntax"
        Translate = "Translate"
        Coref = "Coref"
        Compare = "Compare"
        OpenSmile = "OpenSmile"
        Avqi = "Avqi"

    class ProgressKind(enum.Enum):
        StageStarted = "StageStarted"
        StageInjected = "StageInjected"
        StageFailed = "StageFailed"
        StageSkipped = "StageSkipped"
        SourceCompleted = "SourceCompleted"

    class ProgressEvent:
        def __init__(self, *, source_id, kind, task=None,
                     completed=0, total=0, label=""):
            self.source_id = source_id
            self.kind = kind
            self.task = task
            self.completed = completed
            self.total = total
            self.label = label

    fake = types.ModuleType("batchalign._core")
    fake.Task = Task
    fake.ProgressKind = ProgressKind
    fake.ProgressEvent = ProgressEvent
    monkeypatch.setitem(sys.modules, "batchalign._core", fake)
    return Task, ProgressKind, ProgressEvent
