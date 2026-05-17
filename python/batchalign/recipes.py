"""Recipe layer: user-readable composition rules.

Each recipe is a function that returns a configured `ba.Pipeline`. A recipe
declares the task chain (just a list of `Task` enum values — runners are
canonical and stateless, so there's no per-task config dict) and supplies
the backend instances. Per-pipeline tunables (target language, retokenize,
num_speakers, feature_set, ...) live on the backend constructors.

Recipes lazy-import `Pipeline` and `Task` from `batchalign._core` so a
fresh clone without `maturin develop` can still import this module for
inspection.
"""

from __future__ import annotations

from typing import Any


def _core():
    """Return (Task, Pipeline) from the compiled extension."""
    from batchalign._core import Task, Pipeline  # type: ignore[attr-defined]
    return Task, Pipeline


def transcribe(
    *,
    asr_backend: Any,
    speaker_backend: Any | None = None,
    **opts: Any,
) -> Any:
    """ASR + utterance segmentation (+ optional speaker diarization).

    Force-alignment is *not* wired here on purpose — that's a separate
    `align(fa_backend=...)` recipe you compose after transcription if you
    want refined word-level timings. The asr backend's language /
    num_speakers pins go on its own constructor.
    """
    Task, Pipeline = _core()
    tasks = [Task.Asr]
    backends = [asr_backend]
    if speaker_backend is not None:
        tasks.append(Task.Speaker)
        backends.append(speaker_backend)
    tasks.append(Task.UtSeg)
    return Pipeline(tasks=tasks, backends=backends, **opts)


def align(*, fa_backend: Any, **opts: Any) -> Any:
    """Forced alignment only (`Task.Fa`)."""
    Task, Pipeline = _core()
    return Pipeline(tasks=[Task.Fa], backends=[fa_backend], **opts)


def morphotag(*, stanza_backend: Any, **opts: Any) -> Any:
    """Morphosyntax tagging via Stanza (UD `%mor` / `%gra`)."""
    Task, Pipeline = _core()
    return Pipeline(tasks=[Task.Morphosyntax], backends=[stanza_backend], **opts)


def translate(*, translate_backend: Any, **opts: Any) -> Any:
    """Translate utterances. Target language is set on the backend
    (`GoogleTranslateBackend(target="eng")`)."""
    Task, Pipeline = _core()
    return Pipeline(tasks=[Task.Translate], backends=[translate_backend], **opts)


def coref(*, coref_backend: Any, **opts: Any) -> Any:
    """Coreference resolution."""
    Task, Pipeline = _core()
    return Pipeline(tasks=[Task.Coref], backends=[coref_backend], **opts)


def utseg(*, utseg_backend: Any, **opts: Any) -> Any:
    """Utterance segmentation. `stanza_fallback` lives on the backend."""
    Task, Pipeline = _core()
    return Pipeline(tasks=[Task.UtSeg], backends=[utseg_backend], **opts)


def compare(
    *,
    stanza_backend: Any,
    compare_backend: Any = None,
    **opts: Any,
) -> Any:
    """Gold/main transcript comparison.

    Wires ``[Morphosyntax, Compare]``. Morphosyntax runs on both sides of
    the ``Paired`` (the runner short-circuits per-utterance if `%mor:` is
    already present), so the compare backend can lift POS off the `%mor`
    tier to populate ``%xsmor``.

    ``compare_backend`` defaults to the native Rust
    ``batchalign._core.CompareBackend``. ``stanza_backend`` is required
    for `%xsmor` to carry real POS tags.
    """
    Task, Pipeline = _core()
    if compare_backend is None:
        from batchalign._core import CompareBackend  # type: ignore[attr-defined]
        compare_backend = CompareBackend()
    return Pipeline(
        tasks=[Task.Morphosyntax, Task.Compare],
        backends=[stanza_backend, compare_backend],
        **opts,
    )


def opensmile(*, opensmile_backend: Any, **opts: Any) -> Any:
    """OpenSMILE acoustic-feature extraction. `feature_set` is a backend
    constructor arg (`OpenSmileBackend(feature_set="eGeMAPSv02")`)."""
    Task, Pipeline = _core()
    return Pipeline(tasks=[Task.OpenSmile], backends=[opensmile_backend], **opts)


def avqi(*, avqi_backend: Any, **opts: Any) -> Any:
    """Acoustic Voice Quality Index extraction."""
    Task, Pipeline = _core()
    return Pipeline(tasks=[Task.Avqi], backends=[avqi_backend], **opts)


__all__ = [
    "transcribe",
    "align",
    "morphotag",
    "translate",
    "coref",
    "utseg",
    "compare",
    "opensmile",
    "avqi",
]
