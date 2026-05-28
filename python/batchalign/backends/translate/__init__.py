"""Concrete translation backends."""

from __future__ import annotations

from batchalign.backends.translate.google import GoogleTranslateBackend
from batchalign.backends.translate.vllm import VllmTranslateBackend
from batchalign.backends.translate.nllb import NllbTranslateBackend
from batchalign.backends.translate.tencent import TencentTmtBackend
from batchalign.backends.translate.aliyun import AliyunTranslateBackend

__all__ = [
    "GoogleTranslateBackend",
    "VllmTranslateBackend",
    "NllbTranslateBackend",
    "TencentTmtBackend",
    "AliyunTranslateBackend",
]
