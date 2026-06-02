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
    utseg_backend: Any | None = None,
    **opts: Any,
) -> Any:
    """ASR + utterance segmentation (+ optional speaker diarization).

    This is BA2's transcribe *pairing*: ASR, then a utterance-segmentation
    stage. Pass `utseg_backend=CHATUtteranceBackend(...)` for BA2's BERT
    segmenter (the parity path, applied uniformly to whichever ASR engine
    produced the words). Pyannote, if given as `speaker_backend`, services
    both Speaker and UtSeg, so it covers segmentation on its own.

    Force-alignment is *not* wired here — compose `align(fa_backend=...)`
    afterwards for refined word-level timings.
    """
    Task, Pipeline = _core()
    tasks = [Task.Asr]
    backends = [asr_backend]
    if speaker_backend is not None:
        tasks.append(Task.Speaker)
        backends.append(speaker_backend)
    if utseg_backend is not None:
        # Explicit utterance segmenter (e.g. CHATUtterance) handles UtSeg.
        tasks.append(Task.UtSeg)
        backends.append(utseg_backend)
    elif speaker_backend is not None:
        # Pyannote services Speaker AND UtSeg — UtSeg rides along.
        tasks.append(Task.UtSeg)
    # With neither, there is nothing to serve UtSeg, so we omit it and the ASR
    # segments stand as the utterances (no segmentation).
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
    stanza_backend: Any | None = None,
    compare_backend: Any = None,
    **opts: Any,
) -> Any:
    """Gold/main transcript comparison.

    Wires ``[Morphosyntax, Compare]`` when ``stanza_backend`` is given so
    POS tags reach the ``%xsmor`` tier (the morphosyntax runner short-
    circuits per-utterance when ``%mor:`` is already present, so the
    cost is paid only when needed). With ``stanza_backend=None`` the
    pipeline collapses to ``[Compare]`` — POS tags are read off any
    pre-existing ``%mor:`` in the input, or fall back to ``?``.

    ``compare_backend`` defaults to the native Rust
    ``batchalign._core.backends.CompareBackend``.
    """
    Task, Pipeline = _core()
    if compare_backend is None:
        from batchalign._core.backends import CompareBackend  # type: ignore[attr-defined]
        compare_backend = CompareBackend()
    tasks = []
    backends = []
    if stanza_backend is not None:
        tasks.append(Task.Morphosyntax)
        backends.append(stanza_backend)
    tasks.append(Task.Compare)
    backends.append(compare_backend)
    return Pipeline(tasks=tasks, backends=backends, **opts)


__all__ = [
    "transcribe",
    "align",
    "morphotag",
    "translate",
    "coref",
    "utseg",
    "compare",
]
