"""Recipe layer: user-readable composition rules.

Each recipe is a function that returns a configured `ba.Pipeline`. New
command = new function. See `spec2.md` §16.3 for the canonical list.

Every recipe takes `language=...` and forwards it to tasks that
respect it. Tasks that read per-file from `@Languages` (FA, Stanza,
Translate-source, Coref) either ignore the kwarg or pin `"per-file"`
explicitly. Recipe authors don't need to memorize which tasks are
per-file; pass `language` always and the runner-side config decides.

Recipes lazy-import `Pipeline` and `Task` from `batchalign._core` so a
fresh clone without `maturin develop` can still import this module
for inspection.
"""

from __future__ import annotations

from typing import Any


def _core():
    """Return (Task, Pipeline) from the compiled extension.

    Deferred to call-time so module import succeeds without the .so.
    """
    from batchalign._core import Task, Pipeline  # type: ignore[attr-defined]
    return Task, Pipeline


def transcribe(
    *,
    asr_backend: Any,
    fa_backend: Any | None = None,
    speaker_backend: Any | None = None,
    language: str = "auto",
    num_speakers: int = 0,
    **opts: Any,
) -> Any:
    """ASR (+ optional speaker diarization, FA, utterance segmentation)."""
    Task, Pipeline = _core()
    tasks: list[tuple[Any, dict[str, Any]]] = [
        (Task.Asr, {"language": language, "options": {"num_speakers": num_speakers}})
    ]
    backends = [asr_backend]
    if speaker_backend is not None:
        tasks.append((Task.Speaker, {}))
        backends.append(speaker_backend)
    tasks.append((Task.UtSeg, {"language": language}))
    if fa_backend is not None:
        tasks.append((Task.Fa, {}))   # PerFile from @Languages
        backends.append(fa_backend)
    return Pipeline(tasks=tasks, backends=backends, **opts)


def align(*, fa_backend: Any, **opts: Any) -> Any:
    """Forced alignment only (`Task.Fa`)."""
    Task, Pipeline = _core()
    return Pipeline(
        tasks=[(Task.Fa, {})],
        backends=[fa_backend],
        **opts,
    )


def morphotag(*, stanza_backend: Any, **opts: Any) -> Any:
    """Morphosyntax tagging via Stanza (UD `%mor` / `%gra`)."""
    Task, Pipeline = _core()
    return Pipeline(
        tasks=[(Task.Morphosyntax, {"language": "per-file"})],
        backends=[stanza_backend],
        **opts,
    )


def translate(*, translate_backend: Any, target: str = "eng", **opts: Any) -> Any:
    """Translate utterances; source language read per-file."""
    Task, Pipeline = _core()
    return Pipeline(
        tasks=[(Task.Translate, {"target": target, "source": "per-file"})],
        backends=[translate_backend],
        **opts,
    )


def coref(*, coref_backend: Any, **opts: Any) -> Any:
    """Coreference resolution."""
    Task, Pipeline = _core()
    return Pipeline(
        tasks=[(Task.Coref, {})],
        backends=[coref_backend],
        **opts,
    )


def utseg(
    *,
    utseg_backend: Any,
    stanza_fallback: bool = False,
    **opts: Any,
) -> Any:
    """Utterance segmentation (with optional Stanza punctuation fallback)."""
    Task, Pipeline = _core()
    return Pipeline(
        tasks=[(Task.UtSeg, {"stanza_fallback": stanza_fallback})],
        backends=[utseg_backend],
        **opts,
    )


def compare(**opts: Any) -> Any:
    """Pure-AST gold/main comparison. No backend required."""
    Task, Pipeline = _core()
    return Pipeline(
        tasks=[(Task.Compare, {})],
        backends=[],
        **opts,
    )


def opensmile(
    *,
    opensmile_backend: Any,
    feature_set: str = "eGeMAPSv02",
    **opts: Any,
) -> Any:
    """OpenSMILE acoustic-feature extraction."""
    Task, Pipeline = _core()
    return Pipeline(
        tasks=[(Task.OpenSmile, {"feature_set": feature_set})],
        backends=[opensmile_backend],
        **opts,
    )


def avqi(*, avqi_backend: Any, **opts: Any) -> Any:
    """Acoustic Voice Quality Index extraction."""
    Task, Pipeline = _core()
    return Pipeline(
        tasks=[(Task.Avqi, {})],
        backends=[avqi_backend],
        **opts,
    )


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
