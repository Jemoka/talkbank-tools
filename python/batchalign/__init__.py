"""Batchalign: TalkBank CHAT processing pipeline.

The public surface re-exports two layers:

1. Types and orchestration primitives from the compiled Rust extension
   (`batchalign._core`). If the .so is not built yet (e.g. fresh clone
   without `maturin develop` run), importing this module emits a
   helpful `ImportError` rather than a cryptic missing-symbol failure.

2. Python-side backends, recipes, and CLI helpers.

See `spec2.md` §16 for the canonical surface map.

For an end-user overview of the CLI surface and install extras, see
`python/batchalign/README.md`. For the BA3 cutover plan + per-landing
status, see `book/src/batchalign/developer/landing-status.md`.
"""

from __future__ import annotations

# ---------------------------------------------------------------------------
# Core re-exports from the PyO3 extension. We import explicitly so a missing
# .so produces a single readable diagnostic, not a wall of NameErrors later.
# ---------------------------------------------------------------------------
try:
    from batchalign._core import (  # type: ignore[attr-defined]
        Task,
        Pipeline,
        BAValue,
        MediaInput,
        ChatInput,
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
    _CORE_AVAILABLE = True
except ImportError as _exc:  # pragma: no cover - exercised only without .so
    _CORE_IMPORT_ERROR = _exc
    _CORE_AVAILABLE = False

    # We deliberately don't shadow names with stubs — calling code should
    # fail loudly. But we expose a helper for callers that want to probe.
    def _core_unavailable(*_args, **_kwargs):
        raise ImportError(
            "batchalign._core (the compiled Rust extension) is not built. "
            "Run `cd python && maturin develop` to build it, then re-import. "
            f"Original error: {_CORE_IMPORT_ERROR!r}"
        )

# ---------------------------------------------------------------------------
# Backends re-export. These are pure Python and do not require _core to load,
# although individual backend constructors will lazy-import their ML deps.
# ---------------------------------------------------------------------------
from batchalign.backends import (  # noqa: E402
    Backend,
    ASR,
    FA,
    Speaker,
    UtSeg,
    Morphosyntax,
    Translate,
    Coref,
    OpenSmile,
    AVQI,
    # ASR engines (BA2 parity: rev / whisper / whisperx / openai whisper,
    # plus the Chinese/Cantonese cloud + local engines).
    WhisperBackend,
    ChatWhisperBackend,
    OpenAIWhisperBackend,
    RevAI,
    AliyunAsrBackend,
    FunAsrBackend,
    FunAudioBackend,
    QwenAsrBackend,
    Qwen3AsrBackend,
    TencentAsrBackend,
    # Forced alignment.
    Wav2Vec2FaBackend,
    WhisperFaBackend,
    Qwen3FaBackend,
    # Morphosyntax / speaker / utterance-seg / translate.
    StanzaBackend,
    PyannoteBackend,
    CantoneseWordSegBackend,
    CHATUtteranceBackend,
    GoogleTranslateBackend,
    NllbTranslateBackend,
    TencentTmtBackend,
    AliyunTranslateBackend,
)
from batchalign import recipes  # noqa: E402
from batchalign import inputs  # noqa: E402

__all__ = [
    # _core types
    "Task",
    "Pipeline",
    "BAValue",
    "MediaInput",
    "ChatInput",
    "PairedInput",
    "CacheSpec",
    "CachePolicy",
    "BatchPolicy",
    "CompareBackend",
    "ProgressEvent",
    "ProgressKind",
    "nuke_cache",
    "default_cache_path",
    # backend ABCs
    "Backend",
    "ASR",
    "FA",
    "Speaker",
    "UtSeg",
    "Morphosyntax",
    "Translate",
    "Coref",
    "OpenSmile",
    "AVQI",
    # concrete backends — ASR
    "WhisperBackend",
    "ChatWhisperBackend",
    "OpenAIWhisperBackend",
    "RevAI",
    "AliyunAsrBackend",
    "FunAsrBackend",
    "FunAudioBackend",
    "QwenAsrBackend",
    "Qwen3AsrBackend",
    "TencentAsrBackend",
    # FA
    "Wav2Vec2FaBackend",
    "WhisperFaBackend",
    "Qwen3FaBackend",
    # morphosyntax / speaker / utseg / translate
    "StanzaBackend",
    "PyannoteBackend",
    "CantoneseWordSegBackend",
    "CHATUtteranceBackend",
    "GoogleTranslateBackend",
    "NllbTranslateBackend",
    "TencentTmtBackend",
    "AliyunTranslateBackend",
    # submodules
    "recipes",
    "inputs",
]
