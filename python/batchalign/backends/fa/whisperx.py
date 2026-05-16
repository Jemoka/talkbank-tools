"""Forced-alignment alias for :class:`WhisperXBackend`.

The WhisperX backend handles both ASR and FA via a single class that
inherits from both markers (see :mod:`batchalign.backends.asr.whisperx`).
We re-export it here under the ``fa/`` namespace so callers reaching
into ``batchalign.backends.fa`` find it where they expect.

``WhisperXFaBackend`` is an alias, not a subclass — there's exactly one
loaded model.
"""

from __future__ import annotations

from batchalign.backends.asr.whisperx import WhisperXBackend

WhisperXFaBackend = WhisperXBackend

__all__ = ["WhisperXFaBackend"]
