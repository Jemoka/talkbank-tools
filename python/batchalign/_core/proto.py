"""
!!!  HAND-MIRRORED with crates/batchalign/batchalign-core/src/proto/  !!!

Edits to ``AsrInput``, ``AsrOutput``, ``FaInput``, etc. MUST be made in the
matching Rust files. Tests in
``crates/batchalign/batchalign-core/tests/proto_parity.rs`` will fail if a
class disappears here, but they do not check field-level shape — that
discipline is on the contributor.

See ``spec2.md`` §18 for the parity discipline.

Once the schemars JSON-Schema export stabilises this file becomes generated;
until then keep it hand-edited and tightly coupled to the Rust crate.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Literal, Optional


# ---------------------------------------------------------------------------
# Shared
# ---------------------------------------------------------------------------


@dataclass
class PreparedAudio:
    """Mirrors crates/batchalign/batchalign-core/src/media.rs::PreparedAudio."""

    pcm_f32le: bytes = b""
    sample_rate: int = 16000
    channels: int = 1
    frame_count: int = 0


@dataclass
class LanguageSpec:
    """Mirrors crates/batchalign/batchalign-core/src/proto/asr.rs::LanguageSpec.

    Tagged enum: ``kind`` is one of ``"auto"``, ``"code"``, ``"per_file"``.
    ``value`` is the ISO-639-3 code when ``kind == "code"``, else ``None``.
    """

    kind: Literal["auto", "code", "per_file"] = "auto"
    value: Optional[str] = None


# ---------------------------------------------------------------------------
# ASR
# ---------------------------------------------------------------------------


@dataclass
class AsrOptions:
    num_speakers: int = 0
    prompt: Optional[str] = None
    extras: Any = None


@dataclass
class AsrWord:
    text: str = ""
    start_ms: int = 0
    end_ms: int = 0
    confidence: Optional[float] = None


@dataclass
class AsrSegment:
    start_ms: int = 0
    end_ms: int = 0
    text: str = ""
    speaker: Optional[str] = None
    words: list[AsrWord] = field(default_factory=list)


@dataclass
class AsrInput:
    source_id: str = ""
    audio: PreparedAudio = field(default_factory=PreparedAudio)
    language: LanguageSpec = field(default_factory=LanguageSpec)
    options: AsrOptions = field(default_factory=AsrOptions)


@dataclass
class AsrOutput:
    source_id: str = ""
    segments: list[AsrSegment] = field(default_factory=list)


# ---------------------------------------------------------------------------
# Forced alignment
# ---------------------------------------------------------------------------


@dataclass
class FaInput:
    source_id: str = ""
    audio: PreparedAudio = field(default_factory=PreparedAudio)
    utterances: list[AsrSegment] = field(default_factory=list)
    language: LanguageSpec = field(default_factory=LanguageSpec)


@dataclass
class FaOutput:
    source_id: str = ""
    utterances: list[AsrSegment] = field(default_factory=list)


# ---------------------------------------------------------------------------
# Speaker diarization
# ---------------------------------------------------------------------------


@dataclass
class DiarizationSegment:
    start_ms: int = 0
    end_ms: int = 0
    speaker: str = ""


@dataclass
class Diarization:
    segments: list[DiarizationSegment] = field(default_factory=list)


@dataclass
class SpeakerInput:
    source_id: str = ""
    audio: PreparedAudio = field(default_factory=PreparedAudio)
    num_speakers: int = 0


@dataclass
class SpeakerOutput:
    source_id: str = ""
    diarization: Diarization = field(default_factory=Diarization)


# ---------------------------------------------------------------------------
# Utterance segmentation
# ---------------------------------------------------------------------------


@dataclass
class UtteranceSpan:
    start_ms: int = 0
    end_ms: int = 0
    text: str = ""
    words: list[AsrWord] = field(default_factory=list)


@dataclass
class UtSegInput:
    source_id: str = ""
    segments: list[AsrSegment] = field(default_factory=list)
    language: LanguageSpec = field(default_factory=LanguageSpec)
    stanza_fallback: bool = False


@dataclass
class UtSegOutput:
    source_id: str = ""
    utterances: list[UtteranceSpan] = field(default_factory=list)


# ---------------------------------------------------------------------------
# Morphosyntax
# ---------------------------------------------------------------------------


@dataclass
class MorphosyntaxUtterance:
    speaker: str = ""
    text: str = ""


@dataclass
class MorphosyntaxToken:
    text: str = ""
    lemma: str = ""
    upos: str = ""
    features: list[str] = field(default_factory=list)
    head: Optional[int] = None
    deprel: Optional[str] = None


@dataclass
class TaggedUtterance:
    speaker: str = ""
    tokens: list[MorphosyntaxToken] = field(default_factory=list)
    mor: Optional[str] = None
    gra: Optional[str] = None


@dataclass
class MorphosyntaxInput:
    """Per-utterance input. One dispatched per main-tier utterance."""

    source_id: str = ""
    utterance_id: int = 0
    language: LanguageSpec = field(default_factory=LanguageSpec)
    tokens: list[str] = field(default_factory=list)
    retokenize: bool = False
    text: str = ""


@dataclass
class MorphosyntaxOutput:
    """Per-utterance output, paired index-wise with the input."""

    source_id: str = ""
    utterance_id: int = 0
    tokens: list[MorphosyntaxToken] = field(default_factory=list)
    mor: Optional[str] = None
    gra: Optional[str] = None


# ---------------------------------------------------------------------------
# Translate
# ---------------------------------------------------------------------------


@dataclass
class TranslateInput:
    source_id: str = ""
    utterances: list[str] = field(default_factory=list)
    source: LanguageSpec = field(default_factory=LanguageSpec)
    target: str = "eng"


@dataclass
class TranslateOutput:
    source_id: str = ""
    utterances: list[str] = field(default_factory=list)


# ---------------------------------------------------------------------------
# Coreference
# ---------------------------------------------------------------------------


@dataclass
class CorefInput:
    source_id: str = ""
    utterances: list[str] = field(default_factory=list)
    speakers: list[str] = field(default_factory=list)


@dataclass
class CorefOutput:
    source_id: str = ""
    annotations: list[str] = field(default_factory=list)


# ---------------------------------------------------------------------------
# openSMILE
# ---------------------------------------------------------------------------


@dataclass
class OpenSmileInput:
    source_id: str = ""
    audio: PreparedAudio = field(default_factory=PreparedAudio)
    feature_set: str = "eGeMAPSv02"


@dataclass
class OpenSmileOutput:
    source_id: str = ""
    feature_set: str = "eGeMAPSv02"
    table: dict[str, Any] = field(default_factory=dict)


# ---------------------------------------------------------------------------
# AVQI
# ---------------------------------------------------------------------------


@dataclass
class AvqiInput:
    source_id: str = ""
    audio: PreparedAudio = field(default_factory=PreparedAudio)


@dataclass
class AvqiOutput:
    source_id: str = ""
    table: dict[str, Any] = field(default_factory=dict)
