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


# ---------------------------------------------------------------------------
# Compare (hand-mirrored with crates/batchalign-core/src/proto/compare.rs).
# ---------------------------------------------------------------------------


@dataclass
class CompareInput:
    source_id: str = ""
    main_chat: str = ""
    gold_chat: str = ""


@dataclass
class CompareMetricsPos:
    pos: str = ""
    matches: int = 0
    insertions: int = 0
    deletions: int = 0
    total: int = 0


@dataclass
class CompareMetrics:
    file_label: str = ""
    wer: float = 0.0
    accuracy: float = 0.0
    matches: int = 0
    insertions: int = 0
    deletions: int = 0
    total_gold_words: int = 0
    total_main_words: int = 0
    per_pos: list[CompareMetricsPos] = field(default_factory=list)


@dataclass
class CompareOutput:
    source_id: str = ""
    annotated_main: str = ""
    metrics_json: str = ""
    metrics: CompareMetrics = field(default_factory=CompareMetrics)


# ---------------------------------------------------------------------------
# Tagged-dict → typed dataclass rehydration. The engine ships a Rust-serde
# JSON of `Vec<TaskInput>` (tag "task", content "data") to Python; we
# reconstitute each item to its typed proto class here so backend `call()`
# methods see e.g. a real `MorphosyntaxInput` (with a real `LanguageSpec`
# nested) instead of plain dicts.
# ---------------------------------------------------------------------------

import dataclasses
from typing import get_type_hints, get_origin, get_args, Union


_TAG_TO_INPUT: dict[str, type] = {
    "Asr": AsrInput,
    "Fa": FaInput,
    "Speaker": SpeakerInput,
    "UtSeg": UtSegInput,
    "Morphosyntax": MorphosyntaxInput,
    "Translate": TranslateInput,
    "Coref": CorefInput,
    "OpenSmile": OpenSmileInput,
    "Avqi": AvqiInput,
    "Compare": CompareInput,
}

_TAG_TO_OUTPUT: dict[str, type] = {
    "Asr": AsrOutput,
    "Fa": FaOutput,
    "Speaker": SpeakerOutput,
    "UtSeg": UtSegOutput,
    "Morphosyntax": MorphosyntaxOutput,
    "Translate": TranslateOutput,
    "Coref": CorefOutput,
    "OpenSmile": OpenSmileOutput,
    "Avqi": AvqiOutput,
    "Compare": CompareOutput,
}


def _rebuild(cls: Any, data: Any) -> Any:
    """Recursively reshape a JSON-decoded value into typed dataclasses.

    Handles dataclasses, lists, and `Optional[T]` unions. Bytes payloads
    that travel as base64 strings on the JSON side are left alone — the
    `pcm_f32le` field on `PreparedAudio` is typed `bytes` but Rust's serde
    serialises it as an array of integers; we accept whatever shape it
    arrives in.
    """
    if cls is Any or cls is None or data is None:
        return data
    origin = get_origin(cls)
    if origin is list:
        (inner,) = get_args(cls)
        return [_rebuild(inner, x) for x in data]
    if origin is Union:
        # Optional[T] = Union[T, None]; pick the first non-None arm.
        for arm in get_args(cls):
            if arm is type(None):
                continue
            try:
                return _rebuild(arm, data)
            except (TypeError, ValueError):
                continue
        return data
    if dataclasses.is_dataclass(cls) and isinstance(data, dict):
        hints = get_type_hints(cls)
        kwargs: dict[str, Any] = {}
        for f in dataclasses.fields(cls):
            if f.name in data:
                kwargs[f.name] = _rebuild(hints.get(f.name, Any), data[f.name])
        return cls(**kwargs)
    return data


def rebuild_tagged_inputs(items: list[dict]) -> list[Any]:
    """Convert a list of `{"task": tag, "data": {...}}` dicts (as produced
    by Rust serde's `TaskInput` tagged-enum serialisation) into typed proto
    dataclass instances ready to feed a Python `Backend.call()`."""
    out: list[Any] = []
    for item in items:
        tag = item.get("task")
        cls = _TAG_TO_INPUT.get(tag)
        if cls is None:
            raise TypeError(f"unknown TaskInput tag {tag!r}")
        out.append(_rebuild(cls, item.get("data", {})))
    return out


def serialize_tagged_outputs(items: list[Any]) -> list[dict]:
    """Inverse of `rebuild_tagged_inputs`: take a list of typed
    `*Output` dataclass instances and emit the tagged-dict shape Rust
    serde expects on the way back."""
    out_to_tag = {cls: tag for tag, cls in _TAG_TO_OUTPUT.items()}
    out: list[dict] = []
    for item in items:
        tag = out_to_tag.get(type(item))
        if tag is None:
            raise TypeError(f"unknown Output dataclass {type(item).__name__}")
        out.append({"task": tag, "data": dataclasses.asdict(item)})
    return out
