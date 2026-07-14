"""Backend layer for batchalign.

Backends are grouped by task into subpackages so a reader looking for the
ASR family lands in `batchalign.backends.asr`, FA in `batchalign.backends.fa`,
and so on. The top-level re-exports below preserve the flat public surface
so existing code can keep writing::

    from batchalign.backends import WhisperBackend, StanzaBackend

See `spec2.md` §10 for the marker-ABC design.
"""

from __future__ import annotations

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
    declared_tasks,
)
from batchalign.backends.ai import DspyAIBackend
from batchalign.backends.asr import (
    AliyunAsrBackend,
    ChatWhisperBackend,
    FunAsrBackend,
    FunAudioBackend,
    GoogleGenAIBackend,
    MalayalamWav2Vec2Backend,
    OpenAIWhisperBackend,
    QwenAsrBackend,
    Qwen3AsrBackend,
    RevAI,
    TencentAsrBackend,
    WhisperBackend,
)
from batchalign.backends.fa import (
    Qwen3FaBackend,
    Wav2Vec2FaBackend,
    WhisperFaBackend,
)
from batchalign.backends.morphosyntax import StanzaBackend
from batchalign.backends.speaker import PyannoteBackend
from batchalign.backends.translate import (
    AliyunTranslateBackend,
    GoogleTranslateBackend,
    NllbTranslateBackend,
    TencentTmtBackend,
)
from batchalign.backends.utseg import (
    CantoneseWordSegBackend,
    CHATUtteranceBackend,
    MalayalamSaTBackend,
)

__all__ = [
    # Marker ABCs
    "Backend",
    "AI",
    "ASR",
    "FA",
    "Speaker",
    "UtSeg",
    "Morphosyntax",
    "Translate",
    "Coref",
    "declared_tasks",
    # AI
    "DspyAIBackend",
    # ASR
    "AliyunAsrBackend",
    "ChatWhisperBackend",
    "FunAsrBackend",
    "FunAudioBackend",
    "GoogleGenAIBackend",
    "MalayalamWav2Vec2Backend",
    "OpenAIWhisperBackend",
    "QwenAsrBackend",
    "Qwen3AsrBackend",
    "RevAI",
    "TencentAsrBackend",
    "WhisperBackend",
    # FA
    "Wav2Vec2FaBackend",
    "WhisperFaBackend",
    "Qwen3FaBackend",
    # Morphosyntax
    "StanzaBackend",
    # Speaker
    "PyannoteBackend",
    # Translate
    "AliyunTranslateBackend",
    "GoogleTranslateBackend",
    "NllbTranslateBackend",
    "TencentTmtBackend",
    # UtSeg
    "CantoneseWordSegBackend",
    "CHATUtteranceBackend",
    "MalayalamSaTBackend",
]
