"""Concrete utterance-segmentation backends."""

from __future__ import annotations

from batchalign.backends.utseg.cantonese import CantoneseWordSegBackend
from batchalign.backends.utseg.chatutterance import CHATUtteranceBackend

__all__ = ["CantoneseWordSegBackend", "CHATUtteranceBackend"]
