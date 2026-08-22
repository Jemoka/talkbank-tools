"""Concrete speaker-diarization backends."""

from __future__ import annotations

from batchalign.backends.speaker.pyannote import PyannoteBackend
from batchalign.backends.speaker.pyannote_ai import PyannoteAIBackend

__all__ = ["PyannoteAIBackend", "PyannoteBackend"]
