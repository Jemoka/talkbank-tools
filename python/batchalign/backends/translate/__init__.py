"""Concrete translation backends."""

from __future__ import annotations

from batchalign.backends.translate.google import GoogleTranslateBackend
from batchalign.backends.translate.vllm import VllmTranslateBackend

__all__ = ["GoogleTranslateBackend", "VllmTranslateBackend"]
