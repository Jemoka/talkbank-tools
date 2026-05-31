"""Concrete forced-alignment backends."""

from __future__ import annotations

from batchalign.backends.fa.wav2vec2 import Wav2Vec2FaBackend
from batchalign.backends.fa.whisper_fa import WhisperFaBackend
from batchalign.backends.fa.whisperx import WhisperXFaBackend
from batchalign.backends.fa.qwen3_fa import Qwen3FaBackend

__all__ = [
    "Wav2Vec2FaBackend",
    "WhisperFaBackend",
    "WhisperXFaBackend",
    "Qwen3FaBackend",
]
