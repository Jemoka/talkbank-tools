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
        Ai = "Ai"
        Asr = "Asr"
        Fa = "Fa"
        Speaker = "Speaker"
        UtSeg = "UtSeg"
        Utr = "Utr"
        Morphosyntax = "Morphosyntax"
        Translate = "Translate"
        Coref = "Coref"
        Compare = "Compare"
        Convert = "Convert"

    captured = {}

    class Pipeline:
        def __init__(self, *, tasks, backends, **opts):
            self.tasks = list(tasks)
            self.backends = list(backends)
            self.opts = opts
            captured["last"] = self

    class CompareBackend:
        """Stand-in for `batchalign._core.backends.CompareBackend`."""
        name = "compare:rust:v3.1"

    class ConvertBackend:
        """Stand-in for `batchalign._core.backends.ConvertBackend`."""

        def __init__(self, format):
            self.format = format
            self.name = f"convert:rust:{format}"

    class CacheSpec:
        @staticmethod
        def bypass():
            return "cache-bypass"

    fake = types.ModuleType("batchalign._core")
    fake.Task = Task
    fake.Pipeline = Pipeline
    fake.CacheSpec = CacheSpec
    fake_backends = types.ModuleType("batchalign._core.backends")
    fake_backends.CompareBackend = CompareBackend
    fake_backends.ConvertBackend = ConvertBackend
    fake.backends = fake_backends
    monkeypatch.setitem(sys.modules, "batchalign._core", fake)
    monkeypatch.setitem(sys.modules, "batchalign._core.backends", fake_backends)
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
    assert _task_names(pipe) == ["Asr", "Speaker"]
    assert pipe.backends == ["ASR-stub", "SPK-stub"]


def test_transcribe_with_distinct_diarize_and_utseg(fake_core):
    Task, Pipeline, _ = fake_core
    from batchalign import recipes

    pipe = recipes.transcribe(
        asr_backend="ASR-stub",
        speaker_backend="SPK-stub",
        utseg_backend="UTSEG-stub",
    )
    assert _task_names(pipe) == ["Asr", "Speaker", "UtSeg"]
    assert pipe.backends == ["ASR-stub", "SPK-stub", "UTSEG-stub"]


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


def test_ai(fake_core):
    Task, Pipeline, _ = fake_core
    from batchalign import recipes

    pipe = recipes.ai(ai_backend="ai")
    assert _task_names(pipe) == ["Ai"]
    assert pipe.backends == ["ai"]


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
    assert pipe.backends[1].name == "compare:rust:v3.1"


def test_convert_defaults_to_native_backend_and_bypasses_cache(fake_core):
    Task, Pipeline, _ = fake_core
    from batchalign import recipes

    pipe = recipes.convert(format="mp3", workers=3)
    assert _task_names(pipe) == ["Convert"]
    assert pipe.backends[0].name == "convert:rust:mp3"
    assert pipe.opts == {"workers": 3, "cache": "cache-bypass"}


def test_convert_accepts_explicit_backend_and_cache(fake_core):
    Task, Pipeline, _ = fake_core
    from batchalign import recipes

    backend = object()
    pipe = recipes.convert(
        format="wav",
        convert_backend=backend,
        cache="caller-cache",
    )
    assert pipe.backends == [backend]
    assert pipe.opts["cache"] == "caller-cache"
