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
    diarize: bool = False,
    **opts: Any,
) -> Any:
    """ASR + utterance segmentation (+ optional speaker diarization).

    This is BA2's transcribe *pairing*: ASR, then a utterance-segmentation
    stage. Pass `utseg_backend=CHATUtteranceBackend(...)` for BA2's BERT
    segmenter (the parity path, applied uniformly to whichever ASR engine
    produced the words). A legacy speaker backend that also implements UtSeg
    can cover segmentation on its own; diarization-only services cannot.

    Force-alignment is *not* wired here — compose `align(fa_backend=...)`
    afterwards for refined word-level timings.
    """
    Task, Pipeline = _core()
    tasks = [Task.Asr]
    backends = [asr_backend]
    if speaker_backend is not None or diarize:
        tasks.append(Task.Speaker)
        if speaker_backend is not None and speaker_backend is not asr_backend:
            backends.append(speaker_backend)
    if utseg_backend is not None:
        # Explicit utterance segmenter (e.g. CHATUtterance) handles UtSeg.
        tasks.append(Task.UtSeg)
        backends.append(utseg_backend)
    elif speaker_backend is not None:
        # A legacy speaker backend may also implement UtSeg. Diarization-only
        # services (including pyannoteAI) must not fabricate that capability.
        from batchalign.backends.base import UtSeg

        if isinstance(speaker_backend, UtSeg):
            tasks.append(Task.UtSeg)
    # With neither, there is nothing to serve UtSeg, so we omit it and the ASR
    # segments stand as the utterances (no segmentation).
    return Pipeline(tasks=tasks, backends=backends, **opts)


def diarize(*, speaker_backend: Any, **opts: Any) -> Any:
    """Inject diarized speaker assignments into existing timed CHAT.

    Uses the canonical Speaker runner, including Pipeline caching, progress,
    media preparation, and CHAT mutation.
    """
    Task, Pipeline = _core()
    return Pipeline(
        tasks=[Task.Speaker],
        backends=[speaker_backend],
        **opts,
    )


def align(
    *,
    fa_backend: Any,
    utr_backend: Any | None = None,
    **opts: Any,
) -> Any:
    """Forced alignment, with optional Utterance Timing Recovery pre-pass.

    When ``utr_backend`` is supplied (any ASR backend that subclasses
    :class:`batchalign.backends.base.UTR` — Whisper, ChatWhisper, Rev.AI),
    the pipeline runs ``[Task.Utr, Task.Fa]`` so untimed transcripts have
    their utterance bullets recovered before FA slices the audio. The UTR
    runner skips itself when every utterance is already timed, so passing
    a UTR backend is always safe.

    When ``utr_backend`` is omitted, runs bare FA — appropriate for
    transcripts that already carry utterance bullets (UtSeg output, hand-
    timed CHATs, FA reruns).
    """
    Task, Pipeline = _core()
    if utr_backend is not None:
        return Pipeline(
            tasks=[Task.Utr, Task.Fa],
            backends=[utr_backend, fa_backend],
            **opts,
        )
    return Pipeline(tasks=[Task.Fa], backends=[fa_backend], **opts)


def utr(*, utr_backend: Any, **opts: Any) -> Any:
    """Standalone Utterance Timing Recovery (`Task.Utr`).

    Use this when you want to inject utterance bullets without immediately
    running FA. The backend is any ASR engine that has opted into
    :class:`batchalign.backends.base.UTR` via its MRO (Whisper, ChatWhisper,
    Rev.AI ship with the marker; opt in others by subclassing).
    """
    Task, Pipeline = _core()
    return Pipeline(tasks=[Task.Utr], backends=[utr_backend], **opts)


def morphotag(*, stanza_backend: Any, **opts: Any) -> Any:
    """Morphosyntax tagging via Stanza (UD `%mor` / `%gra`)."""
    Task, Pipeline = _core()
    return Pipeline(tasks=[Task.Morphosyntax], backends=[stanza_backend], **opts)


def translate(*, translate_backend: Any, **opts: Any) -> Any:
    """Translate utterances. Target language is set on the backend
    (`GoogleTranslateBackend(target="eng")`)."""
    Task, Pipeline = _core()
    return Pipeline(tasks=[Task.Translate], backends=[translate_backend], **opts)


def ai(*, ai_backend: Any, **opts: Any) -> Any:
    """Generic AI transcript editing."""
    Task, Pipeline = _core()
    return Pipeline(tasks=[Task.Ai], backends=[ai_backend], **opts)


def coref(*, coref_backend: Any, **opts: Any) -> Any:
    """Coreference resolution."""
    Task, Pipeline = _core()
    return Pipeline(tasks=[Task.Coref], backends=[coref_backend], **opts)


def utseg(*, utseg_backend: Any, **opts: Any) -> Any:
    """Utterance segmentation. `stanza_fallback` lives on the backend."""
    Task, Pipeline = _core()
    return Pipeline(tasks=[Task.UtSeg], backends=[utseg_backend], **opts)


def convert(*, format: str, convert_backend: Any = None, **opts: Any) -> Any:
    """Decode media and produce a new WAV or MP3 artifact.

    The default backend is native Rust. Conversion bypasses the shared LMDB
    cache unless the caller explicitly supplies a cache policy: encoded media
    payloads are large and cheap enough to regenerate that caching them is a
    poor default.
    """
    Task, Pipeline = _core()
    if convert_backend is None:
        from batchalign._core import CacheSpec  # type: ignore[attr-defined]
        from batchalign._core.backends import ConvertBackend  # type: ignore[attr-defined]
        convert_backend = ConvertBackend(format)
        opts.setdefault("cache", CacheSpec.bypass())
    return Pipeline(tasks=[Task.Convert], backends=[convert_backend], **opts)


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
    "diarize",
    "align",
    "utr",
    "morphotag",
    "translate",
    "ai",
    "coref",
    "utseg",
    "compare",
    "convert",
]
