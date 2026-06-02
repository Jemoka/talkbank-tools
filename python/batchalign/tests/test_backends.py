"""Tests for backend ABCs and marker-MRO declared-task logic."""

from __future__ import annotations

import pytest

from batchalign.backends.base import (
    Backend,
    ASR,
    FA,
    Speaker,
    UtSeg,
    Morphosyntax,
    Translate,
    Coref,
    BatchPolicy,
    Task,
    declared_tasks,
)


# A minimal concrete backend used to test declared_tasks across MROs.
class _StubBackend(Backend):
    def __init__(self, name: str = "stub") -> None:
        self._n = name

    @property
    def name(self) -> str:
        return self._n

    @property
    def batch_policy(self) -> BatchPolicy:
        return BatchPolicy(max_size=1, window_ms=0)

    def call(self, batch):
        return []


class _StubAsr(_StubBackend, ASR):
    pass


class _StubAsrFa(_StubBackend, ASR, FA):
    pass


class _StubAsrSpeaker(_StubBackend, ASR, Speaker):
    pass


class _StubMorpho(_StubBackend, Morphosyntax):
    pass


class _StubEverything(
    _StubBackend, ASR, FA, Speaker, UtSeg, Morphosyntax, Translate, Coref
):
    pass


def test_single_marker():
    b = _StubAsr()
    assert declared_tasks(b) == [Task.Asr]


def test_multi_marker_whisper_shape():
    b = _StubAsrFa()
    tasks = declared_tasks(b)
    assert Task.Asr in tasks
    assert Task.Fa in tasks
    assert Task.Speaker not in tasks


def test_atomic_call_revai_shape():
    b = _StubAsrSpeaker()
    tasks = declared_tasks(b)
    assert tasks == [Task.Asr, Task.Speaker]


def test_morphosyntax_marker():
    b = _StubMorpho()
    assert declared_tasks(b) == [Task.Morphosyntax]


def test_every_marker():
    b = _StubEverything()
    tasks = declared_tasks(b)
    assert set(tasks) == {
        Task.Asr,
        Task.Fa,
        Task.Speaker,
        Task.UtSeg,
        Task.Morphosyntax,
        Task.Translate,
        Task.Coref,
    }


def test_backend_abc_cannot_instantiate():
    # Backend itself is abstract.
    with pytest.raises(TypeError):
        Backend()  # type: ignore[abstract]


def test_marker_subclass_without_methods_cannot_instantiate():
    class HollowAsr(ASR):
        pass

    with pytest.raises(TypeError):
        HollowAsr()  # type: ignore[abstract]


def test_batch_policy_constructors():
    one = BatchPolicy.one()
    assert one.max_size == 1
    assert one.window_ms == 0

    fixed = BatchPolicy.fixed(7)
    assert fixed.max_size == 7


def test_backend_class_re_exports():
    # Importing from the package re-export should yield the same classes.
    from batchalign.backends import (
        Backend as B2,
        ASR as ASR2,
        Morphosyntax as M2,
    )
    assert B2 is Backend
    assert ASR2 is ASR
    assert M2 is Morphosyntax


def test_concrete_backend_classes_importable():
    # The concrete classes must at least be importable (their constructors
    # lazy-import heavy ML deps, so just touching the class is fine).
    from batchalign.backends import (
        WhisperBackend,
        StanzaBackend,
        PyannoteBackend,
        GoogleTranslateBackend,
        RevAI,
        TencentAsrBackend,
        AliyunAsrBackend,
        FunAudioBackend,
    )

    # Each must inherit from `Backend` (transitively via its marker ABC).
    for cls in (
        WhisperBackend,
        StanzaBackend,
        PyannoteBackend,
        GoogleTranslateBackend,
        RevAI,
        TencentAsrBackend,
        AliyunAsrBackend,
        FunAudioBackend,
    ):
        assert issubclass(cls, Backend), cls


def test_whisper_declared_tasks_via_class_mro():
    # WhisperBackend is ASR-only — the HF Transformers Whisper pipeline does
    # not produce word-level forced alignment in this rewrite (matches BA2's
    # split between WhisperEngine and Wave2VecFAEngine). Use WhisperXBackend
    # or Wav2Vec2FaBackend for FA.
    from batchalign.backends import WhisperBackend
    assert issubclass(WhisperBackend, ASR)
    assert not issubclass(WhisperBackend, FA)


def test_stanza_declared_tasks_via_class_mro():
    from batchalign.backends import StanzaBackend
    assert issubclass(StanzaBackend, Morphosyntax)


def test_revai_atomic_call_via_class_mro():
    from batchalign.backends import RevAI
    assert issubclass(RevAI, ASR)
    assert issubclass(RevAI, Speaker)
