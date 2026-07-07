"""Tests for backend ABCs and marker-MRO declared-task logic."""

from __future__ import annotations

import pytest

from batchalign.backends.base import (
    Backend,
    AI,
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


class _StubAi(_StubBackend, AI):
    pass


class _StubAsrFa(_StubBackend, ASR, FA):
    pass


class _StubAsrSpeaker(_StubBackend, ASR, Speaker):
    pass


class _StubMorpho(_StubBackend, Morphosyntax):
    pass


class _StubEverything(
    _StubBackend, AI, ASR, FA, Speaker, UtSeg, Morphosyntax, Translate, Coref
):
    pass


def test_ai_marker():
    b = _StubAi()
    assert declared_tasks(b) == [Task.Ai]


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
        Task.Ai,
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
        AI as AI2,
        Backend as B2,
        ASR as ASR2,
        Morphosyntax as M2,
    )
    assert B2 is Backend
    assert AI2 is AI
    assert ASR2 is ASR
    assert M2 is Morphosyntax


def test_concrete_backend_classes_importable():
    # The concrete classes must at least be importable (their constructors
    # lazy-import heavy ML deps, so just touching the class is fine).
    from batchalign.backends import (
        DspyAIBackend,
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
        DspyAIBackend,
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


def test_dspy_ai_declared_task_via_class_mro():
    from batchalign.backends import DspyAIBackend
    assert issubclass(DspyAIBackend, AI)


def test_dspy_ai_backend_name_includes_prompt_hash():
    import hashlib

    from batchalign.backends import DspyAIBackend
    from batchalign.backends.ai.dspy import CHAT_SYSTEM_PROMPT

    backend = DspyAIBackend(module=object())
    prompt_hash = hashlib.blake2s(
        CHAT_SYSTEM_PROMPT.encode("utf-8"), digest_size=4
    ).hexdigest()

    assert backend.name.endswith(f":p{prompt_hash}")


def test_dspy_ai_logs_each_input_and_output(caplog):
    from batchalign._core.proto import AiInput, AiUtterance
    from batchalign.backends import DspyAIBackend

    class Prediction:
        revised = "*PAR:\tHello .\n"

    class Module:
        def __call__(self, **_kwargs):
            return Prediction()

    backend = DspyAIBackend(module=Module())
    item = AiInput(
        source_id="sample",
        instruction="translate to chinese",
        utterances=[
            AiUtterance(
                index=0,
                chat="*PAR:\thello .\n",
                context=["*PAR:\tthere .\n"],
            )
        ],
    )

    with caplog.at_level("DEBUG", logger="batchalign.ai"):
        backend.call([item])

    text = caplog.text
    assert "AI model input" in text
    assert "translate to chinese" in text
    assert "*PAR:\thello ." in text
    assert "AI model output" in text
    assert "*PAR:\tHello ." in text


def test_dspy_ai_joins_revised_blocks():
    from batchalign._core.proto import AiInput, AiUtterance
    from batchalign.backends import DspyAIBackend

    class Prediction:
        revised_blocks = [
            "*LENO:\tI never saw my dad put the belt on .",
            "*LENO:\tI only saw him take it off .",
        ]

    class Module:
        def __call__(self, **_kwargs):
            return Prediction()

    backend = DspyAIBackend(module=Module())
    item = AiInput(
        source_id="sample",
        instruction="split into CHAT utterances",
        utterances=[
            AiUtterance(
                index=0,
                chat="*LENO:\tI never saw my dad put the belt on I only saw him take it off .\n",
                context=[],
            )
        ],
    )

    out = backend.call([item])[0]

    assert len(out.revisions) == 1
    assert out.revisions[0].chat == (
        "*LENO:\tI never saw my dad put the belt on .\n"
        "*LENO:\tI only saw him take it off ."
    )


def test_dspy_ai_retries_with_validation_error():
    from batchalign._core.proto import AiInput, AiUtterance
    from batchalign.backends import DspyAIBackend

    class Prediction:
        def __init__(self, revised):
            self.revised = revised

    class Module:
        def __init__(self):
            self.errors = []

        def __call__(self, **kwargs):
            self.errors.append(kwargs["error"])
            if len(self.errors) == 1:
                return Prediction("*PAR:\tbad 49985_ .")
            return Prediction("*PAR:\tgood .")

    validation_errors = []

    def validator(_source_id, _utterance_index, _current_chat, revised_chat):
        if "bad" in revised_chat:
            validation_errors.append("CHAT parse error: bad dependent tier")
            return validation_errors[-1]
        return None

    module = Module()
    backend = DspyAIBackend(module=module, validator=validator)
    item = AiInput(
        source_id="sample",
        instruction="fix",
        utterances=[
            AiUtterance(
                index=0,
                chat="*PAR:\thello .\n",
                context=[],
            )
        ],
    )

    out = backend.call([item])[0]

    assert len(out.revisions) == 1
    assert out.revisions[0].chat == "*PAR:\tgood ."
    assert module.errors[0] == ""
    assert "CHAT parse error: bad dependent tier" in module.errors[1]


def test_dspy_ai_stops_after_max_validation_attempts():
    from batchalign._core.proto import AiInput, AiUtterance
    from batchalign.backends import DspyAIBackend
    from batchalign.backends.ai.dspy import _MAX_VALIDATION_ATTEMPTS

    class Prediction:
        revised = "*PAR:\tbad 49985_ ."

    class Module:
        def __init__(self):
            self.calls = 0

        def __call__(self, **_kwargs):
            self.calls += 1
            return Prediction()

    def validator(_source_id, _utterance_index, _current_chat, _revised_chat):
        return "CHAT parse error: still invalid"

    module = Module()
    backend = DspyAIBackend(module=module, validator=validator)
    item = AiInput(
        source_id="sample",
        instruction="fix",
        utterances=[
            AiUtterance(
                index=0,
                chat="*PAR:\thello .\n",
                context=[],
            )
        ],
    )

    out = backend.call([item])[0]

    assert out.revisions == []
    assert module.calls == _MAX_VALIDATION_ATTEMPTS
