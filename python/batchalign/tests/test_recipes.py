"""Tests for the recipe layer.

Recipes lazy-import `Pipeline` from `batchalign._core`. When the .so
is not built we mock the import so we can still verify that each
recipe wires the expected `(Task, config)` shape and the right number
of backends.
"""

from __future__ import annotations

import sys
import types
from unittest import mock

import pytest


@pytest.fixture
def fake_core(monkeypatch):
    """Install a fake `batchalign._core` module exposing `Task` + `Pipeline`."""
    # Build a minimal Task enum mirroring the real one.
    import enum

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

    captured = {}

    class Pipeline:
        def __init__(self, *, tasks, backends, **opts):
            self.tasks = list(tasks)
            self.backends = list(backends)
            self.opts = opts
            captured["last"] = self

    fake = types.ModuleType("batchalign._core")
    fake.Task = Task
    fake.Pipeline = Pipeline
    # Don't clobber a real _core if it's importable in the test env.
    monkeypatch.setitem(sys.modules, "batchalign._core", fake)
    return Task, Pipeline, captured


def _task_names(pipeline) -> list[str]:
    return [t.value if hasattr(t, "value") else str(t) for t, _ in pipeline.tasks]


def test_transcribe_default(fake_core):
    Task, Pipeline, _ = fake_core
    from batchalign import recipes

    pipe = recipes.transcribe(asr_backend="ASR-stub")
    # Default: ASR, UtSeg (no FA, no Speaker).
    assert _task_names(pipe) == ["Asr", "UtSeg"]
    assert pipe.backends == ["ASR-stub"]


def test_transcribe_with_fa_and_diarize(fake_core):
    Task, Pipeline, _ = fake_core
    from batchalign import recipes

    pipe = recipes.transcribe(
        asr_backend="ASR-stub",
        fa_backend="FA-stub",
        speaker_backend="SPK-stub",
        language="eng",
        num_speakers=2,
    )
    names = _task_names(pipe)
    assert names == ["Asr", "Speaker", "UtSeg", "Fa"]
    assert pipe.backends == ["ASR-stub", "SPK-stub", "FA-stub"]
    # Language must propagate into the ASR config.
    asr_cfg = pipe.tasks[0][1]
    assert asr_cfg["language"] == "eng"
    assert asr_cfg["options"]["num_speakers"] == 2


def test_align(fake_core):
    Task, Pipeline, _ = fake_core
    from batchalign import recipes

    pipe = recipes.align(fa_backend="fa")
    assert _task_names(pipe) == ["Fa"]
    assert pipe.backends == ["fa"]


def test_morphotag(fake_core):
    Task, Pipeline, _ = fake_core
    from batchalign import recipes

    pipe = recipes.morphotag(stanza_backend="stanza")
    assert _task_names(pipe) == ["Morphosyntax"]
    # Morphotag pins per-file language explicitly.
    assert pipe.tasks[0][1] == {"language": "per-file"}


def test_translate(fake_core):
    Task, Pipeline, _ = fake_core
    from batchalign import recipes

    pipe = recipes.translate(translate_backend="t", target="zho")
    assert _task_names(pipe) == ["Translate"]
    assert pipe.tasks[0][1] == {"target": "zho", "source": "per-file"}


def test_coref(fake_core):
    Task, Pipeline, _ = fake_core
    from batchalign import recipes

    pipe = recipes.coref(coref_backend="cor")
    assert _task_names(pipe) == ["Coref"]


def test_utseg(fake_core):
    Task, Pipeline, _ = fake_core
    from batchalign import recipes

    pipe = recipes.utseg(utseg_backend="u", stanza_fallback=True)
    assert _task_names(pipe) == ["UtSeg"]
    assert pipe.tasks[0][1] == {"stanza_fallback": True}


def test_compare_no_backend(fake_core):
    Task, Pipeline, _ = fake_core
    from batchalign import recipes

    pipe = recipes.compare()
    assert _task_names(pipe) == ["Compare"]
    assert pipe.backends == []


def test_opensmile(fake_core):
    Task, Pipeline, _ = fake_core
    from batchalign import recipes

    pipe = recipes.opensmile(opensmile_backend="o", feature_set="eGeMAPSv02")
    assert _task_names(pipe) == ["OpenSmile"]
    assert pipe.tasks[0][1]["feature_set"] == "eGeMAPSv02"


def test_avqi(fake_core):
    Task, Pipeline, _ = fake_core
    from batchalign import recipes

    pipe = recipes.avqi(avqi_backend="a")
    assert _task_names(pipe) == ["Avqi"]
