"""Tests for the recipe layer.

Recipes lazy-import `Pipeline` from `batchalign._core`. We mock that import
so tests don't need the compiled .so. Pipelines now take a plain list of
`Task` enum values (no per-task config dict) — runners are stateless and
canonical, so we only verify the task order + the backend list.
"""

from __future__ import annotations

import sys
import types
from unittest import mock

import pytest


@pytest.fixture
def fake_core(monkeypatch):
    """Install a fake `batchalign._core` module exposing `Task` + `Pipeline`."""
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

    captured = {}

    class Pipeline:
        def __init__(self, *, tasks, backends, **opts):
            self.tasks = list(tasks)
            self.backends = list(backends)
            self.opts = opts
            captured["last"] = self

    class CompareBackend:
        """Stand-in for the native Rust `batchalign._core.CompareBackend`."""
        name = "compare:rust:v2"

    fake = types.ModuleType("batchalign._core")
    fake.Task = Task
    fake.Pipeline = Pipeline
    fake.CompareBackend = CompareBackend
    monkeypatch.setitem(sys.modules, "batchalign._core", fake)
    return Task, Pipeline, captured


def _task_names(pipeline) -> list[str]:
    return [t.value if hasattr(t, "value") else str(t) for t in pipeline.tasks]


def test_transcribe_default(fake_core):
    Task, Pipeline, _ = fake_core
    from batchalign import recipes

    pipe = recipes.transcribe(asr_backend="ASR-stub")
    # ASR only. UtSeg is NOT appended without a speaker backend: BA3 has no
    # standalone utterance segmenter, and the only UtSeg-capable backend
    # (Pyannote) rides on the speaker stage. Appending UtSeg with nothing to
    # serve it aborts the pipeline at runtime. (No FA either — FA composes via
    # the `align()` recipe after transcription.)
    assert _task_names(pipe) == ["Asr"]
    assert pipe.backends == ["ASR-stub"]


def test_transcribe_with_diarize(fake_core):
    Task, Pipeline, _ = fake_core
    from batchalign import recipes

    pipe = recipes.transcribe(
        asr_backend="ASR-stub",
        speaker_backend="SPK-stub",
    )
    assert _task_names(pipe) == ["Asr", "Speaker", "UtSeg"]
    assert pipe.backends == ["ASR-stub", "SPK-stub"]


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
    assert pipe.backends == ["stanza"]


def test_translate(fake_core):
    Task, Pipeline, _ = fake_core
    from batchalign import recipes

    pipe = recipes.translate(translate_backend="t")
    assert _task_names(pipe) == ["Translate"]
    assert pipe.backends == ["t"]


def test_coref(fake_core):
    Task, Pipeline, _ = fake_core
    from batchalign import recipes

    pipe = recipes.coref(coref_backend="cor")
    assert _task_names(pipe) == ["Coref"]


def test_utseg(fake_core):
    Task, Pipeline, _ = fake_core
    from batchalign import recipes

    pipe = recipes.utseg(utseg_backend="u")
    assert _task_names(pipe) == ["UtSeg"]
    assert pipe.backends == ["u"]


def test_compare_chains_morphosyntax_then_compare(fake_core):
    Task, Pipeline, _ = fake_core
    from batchalign import recipes

    pipe = recipes.compare(stanza_backend="stanza_fake")
    assert _task_names(pipe) == ["Morphosyntax", "Compare"]
    assert len(pipe.backends) == 2
    assert pipe.backends[0] == "stanza_fake"
    assert pipe.backends[1].name == "compare:rust:v2"


