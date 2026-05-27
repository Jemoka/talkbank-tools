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
    ASR,
    FA,
    Speaker,
    UtSeg,
    Morphosyntax,
    Translate,
    Coref,
    OpenSmile,
    AVQI,
    declared_tasks,
)
from batchalign.backends.asr import (
    AliyunAsrBackend,
    ChatWhisperBackend,
    FunAsrBackend,
    FunAudioBackend,
    OpenAIWhisperBackend,
    QwenAsrBackend,
    RevAI,
    TencentAsrBackend,
    VllmAsrBackend,
    WhisperBackend,
    WhisperXBackend,
)
from batchalign.backends.fa import Wav2Vec2FaBackend, WhisperXFaBackend
from batchalign.backends.morphosyntax import StanzaBackend
from batchalign.backends.speaker import PyannoteBackend
from batchalign.backends.translate import GoogleTranslateBackend, VllmTranslateBackend
from batchalign.backends.utseg import CantoneseWordSegBackend, CHATUtteranceBackend

__all__ = [
    # Marker ABCs
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
    "declared_tasks",
    # ASR
    "AliyunAsrBackend",
    "ChatWhisperBackend",
    "FunAsrBackend",
    "FunAudioBackend",
    "OpenAIWhisperBackend",
    "QwenAsrBackend",
    "RevAI",
    "TencentAsrBackend",
    "VllmAsrBackend",
    "WhisperBackend",
    "WhisperXBackend",
    # FA
    "Wav2Vec2FaBackend",
    "WhisperXFaBackend",
    # Morphosyntax
    "StanzaBackend",
    # Speaker
    "PyannoteBackend",
    # Translate
    "GoogleTranslateBackend",
    "VllmTranslateBackend",
    # UtSeg
    "CantoneseWordSegBackend",
    "CHATUtteranceBackend",
]
