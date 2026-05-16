"""Concrete forced-alignment backends."""

from __future__ import annotations

from batchalign.backends.fa.wav2vec2 import Wav2Vec2FaBackend
from batchalign.backends.fa.whisperx import WhisperXFaBackend

__all__ = ["Wav2Vec2FaBackend", "WhisperXFaBackend"]
