"""Concrete ASR backends.

Each submodule lazy-imports its ML dependencies inside ``__init__`` /
``call``, so importing this package costs nothing beyond pure Python.
"""

from __future__ import annotations

from batchalign.backends.asr.whisper import WhisperBackend
from batchalign.backends.asr.chatwhisper import ChatWhisperBackend
from batchalign.backends.asr.rev import RevAI
from batchalign.backends.asr.openai_whisper import OpenAIWhisperBackend
from batchalign.backends.asr.qwen import QwenAsrBackend
from batchalign.backends.asr.tencent import TencentAsrBackend
from batchalign.backends.asr.aliyun import AliyunAsrBackend
from batchalign.backends.asr.funasr import FunAsrBackend
from batchalign.backends.asr.funaudio import FunAudioBackend
from batchalign.backends.asr.qwen3_asr import Qwen3AsrBackend

__all__ = [
    "WhisperBackend",
    "ChatWhisperBackend",
    "RevAI",
    "OpenAIWhisperBackend",
    "QwenAsrBackend",
    "TencentAsrBackend",
    "AliyunAsrBackend",
    "FunAsrBackend",
    "FunAudioBackend",
    "Qwen3AsrBackend",
]
