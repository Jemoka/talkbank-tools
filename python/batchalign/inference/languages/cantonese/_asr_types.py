"""Typed ASR payload models shared across built-in HK/Cantonese engines."""

from __future__ import annotations

from typing import Literal, TypedDict

from batchalign.inference._domain_types import TimestampMs


class AsrElement(TypedDict):
    """One ASR token entry for process_generation_to_chat payloads."""

    type: Literal["text"]
    ts: float | None
    end_ts: float | None
    value: str


class AsrMonologue(TypedDict):
    """One speaker segment for process_generation_to_chat payloads."""

    elements: list[AsrElement]
    speaker: int


class AsrGenerationPayload(TypedDict):
    """Top-level ASR payload consumed by batchalign ASR post-processing."""

    monologues: list[AsrMonologue]


class TimedWord(TypedDict):
    """Word timing payload consumed by ParsedChat.add_utterance_timing."""

    word: str
    start_ms: TimestampMs
    end_ms: TimestampMs


class AliyunSentenceWord(TypedDict, total=False):
    """Aliyun per-word result item received from websocket payloads.

    Deprecated: prefer AliyunWord (Pydantic) for new code.
    """

    text: str
    startTime: int | float | str
    endTime: int | float | str
