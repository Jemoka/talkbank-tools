"""Concrete utterance-segmentation backends."""

from __future__ import annotations

from batchalign.backends.utseg.cantonese import CantoneseWordSegBackend
from batchalign.backends.utseg.chatutterance import CHATUtteranceBackend
from batchalign.backends.utseg.malayalam_sat import MalayalamSaTBackend
from batchalign.backends.utseg.stanza_fallback import StanzaUtSegBackend

__all__ = [
    "CantoneseWordSegBackend",
    "CHATUtteranceBackend",
    "MalayalamSaTBackend",
    "StanzaUtSegBackend",
]
