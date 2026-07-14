"""Batchalign: TalkBank CHAT processing pipeline.

The public surface re-exports two layers:

1. Types and orchestration primitives from the compiled Rust extension
   (`batchalign._core`). If the .so is not built yet (e.g. fresh clone
   without `maturin develop` run), accessing one of these names emits a
   helpful `ImportError` rather than a cryptic missing-symbol failure.

2. Python-side backends, recipes, and CLI helpers.

See `spec2.md` §16 for the canonical surface map.

For an end-user overview of the CLI surface and install extras, see
`python/batchalign/README.md`. For the BA3 cutover plan + per-landing
status, see `book/src/batchalign/developer/landing-status.md`.

Imports are PEP 562 lazy. The package itself is cheap to import; the
PyO3 `.so` and the backend re-exports only materialize on first access.
This keeps `--help`/`--version`/shell-completion paths sub-200ms.
"""

from __future__ import annotations

from typing import TYPE_CHECKING, Any

# Map attribute name → source module. Resolved lazily by __getattr__.
_CORE_NAMES = frozenset({
    "Task",
    "Pipeline",
    "BAValue",
    "MediaInput",
    "ChatInput",
    "AiChatInput",
    "PairedInput",
    "CacheSpec",
    "CachePolicy",
    "BatchPolicy",
    "CompareBackend",
    "ProgressEvent",
    "ProgressKind",
    "nuke_cache",
    "default_cache_path",
})

_BACKEND_NAMES = frozenset({
    "Backend",
    "AI",
    "ASR",
    "FA",
    "Speaker",
    "UtSeg",
    "Morphosyntax",
    "Translate",
    "Coref",
    "DspyAIBackend",
    "WhisperBackend",
    "ChatWhisperBackend",
    "OpenAIWhisperBackend",
    "RevAI",
    "AliyunAsrBackend",
    "FunAsrBackend",
    "FunAudioBackend",
    "GoogleGenAIBackend",
    "MalayalamWav2Vec2Backend",
    "QwenAsrBackend",
    "Qwen3AsrBackend",
    "TencentAsrBackend",
    "Wav2Vec2FaBackend",
    "WhisperFaBackend",
    "Qwen3FaBackend",
    "StanzaBackend",
    "PyannoteBackend",
    "CantoneseWordSegBackend",
    "CHATUtteranceBackend",
    "MalayalamSaTBackend",
    "GoogleTranslateBackend",
    "NllbTranslateBackend",
    "TencentTmtBackend",
    "AliyunTranslateBackend",
})

_SUBMODULES = frozenset({"recipes", "inputs", "backends", "config"})


def __getattr__(name: str) -> Any:
    if name in _CORE_NAMES:
        try:
            from batchalign import _core  # type: ignore[attr-defined]
        except ImportError as exc:
            raise ImportError(
                "batchalign._core (the compiled Rust extension) is not built. "
                "Run `just batchalign::build` to build it, then re-import. "
                f"Original error: {exc!r}"
            ) from exc
        value = getattr(_core, name)
        globals()[name] = value
        return value

    if name in _BACKEND_NAMES:
        import importlib
        backends = importlib.import_module("batchalign.backends")
        value = getattr(backends, name)
        globals()[name] = value
        return value

    if name in _SUBMODULES:
        import importlib
        value = importlib.import_module(f"batchalign.{name}")
        globals()[name] = value
        return value

    raise AttributeError(f"module 'batchalign' has no attribute {name!r}")


def __dir__() -> list[str]:
    return sorted(__all__)


if TYPE_CHECKING:
    # Make IDE/type-checker happy without forcing the runtime imports.
    from batchalign._core import (  # noqa: F401
        Task,
        Pipeline,
        BAValue,
        MediaInput,
        ChatInput,
        AiChatInput,
        PairedInput,
        CacheSpec,
        CachePolicy,
        BatchPolicy,
        CompareBackend,
        ProgressEvent,
        ProgressKind,
        nuke_cache,
        default_cache_path,
    )
    from batchalign.backends import (  # noqa: F401
        Backend,
        AI,
        ASR,
        FA,
        Speaker,
        UtSeg,
        Morphosyntax,
        Translate,
        Coref,
        DspyAIBackend,
        WhisperBackend,
        ChatWhisperBackend,
        OpenAIWhisperBackend,
        RevAI,
        AliyunAsrBackend,
        FunAsrBackend,
        FunAudioBackend,
        GoogleGenAIBackend,
        MalayalamWav2Vec2Backend,
        QwenAsrBackend,
        Qwen3AsrBackend,
        TencentAsrBackend,
        Wav2Vec2FaBackend,
        WhisperFaBackend,
        Qwen3FaBackend,
        StanzaBackend,
        PyannoteBackend,
        CantoneseWordSegBackend,
        CHATUtteranceBackend,
        MalayalamSaTBackend,
        GoogleTranslateBackend,
        NllbTranslateBackend,
        TencentTmtBackend,
        AliyunTranslateBackend,
    )
    from batchalign import recipes, inputs  # noqa: F401


__all__ = [
    "Task",
    "Pipeline",
    "BAValue",
    "MediaInput",
    "ChatInput",
    "AiChatInput",
    "PairedInput",
    "CacheSpec",
    "CachePolicy",
    "BatchPolicy",
    "CompareBackend",
    "ProgressEvent",
    "ProgressKind",
    "nuke_cache",
    "default_cache_path",
    "Backend",
    "AI",
    "ASR",
    "FA",
    "Speaker",
    "UtSeg",
    "Morphosyntax",
    "Translate",
    "Coref",
    "DspyAIBackend",
    "WhisperBackend",
    "ChatWhisperBackend",
    "OpenAIWhisperBackend",
    "RevAI",
    "AliyunAsrBackend",
    "FunAsrBackend",
    "FunAudioBackend",
    "GoogleGenAIBackend",
    "MalayalamWav2Vec2Backend",
    "QwenAsrBackend",
    "Qwen3AsrBackend",
    "TencentAsrBackend",
    "Wav2Vec2FaBackend",
    "WhisperFaBackend",
    "Qwen3FaBackend",
    "StanzaBackend",
    "PyannoteBackend",
    "CantoneseWordSegBackend",
    "CHATUtteranceBackend",
    "MalayalamSaTBackend",
    "GoogleTranslateBackend",
    "NllbTranslateBackend",
    "TencentTmtBackend",
    "AliyunTranslateBackend",
    "recipes",
    "inputs",
]
