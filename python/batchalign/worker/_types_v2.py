"""Typed schema models for worker protocol V2.

These models mirror `crates/batchalign-types/src/worker_v2/` (re-exported via
`crates/batchalign/src/types/worker_v2.rs`). The ``*_v2`` namespace is
intentional: Rust and Python still ship the frozen V1 worker surface
(``worker`` / ``_types.py``), while ``worker_v2`` is the live typed execute
contract, the JSON Schema layer (``schemas/ipc/worker_v2``), and the generated
Python package namespace (``python/batchalign/generated/worker_v2``).

- define the canonical V2 protocol shape in Python
- validate canonical fixture files shared with Rust
- prevent drift while V1 and V2 coexist

The design goal is to make Python a thin model host. Request and response
models therefore describe model-ready inputs plus prepared-artifact references,
not CLI commands or document-processing workflows.
"""

from __future__ import annotations

from enum import Enum
from typing import Annotated, Literal, TypeAlias

from pydantic import BaseModel, Field, FiniteFloat, StringConstraints, model_validator

from batchalign.inference._domain_types import LanguageCode, NumSpeakers, SpeakerId
from batchalign.worker._types import WorkerJSONValue

WorkerRequestIdV2: TypeAlias = Annotated[str, StringConstraints(min_length=1)]
"""Stable identifier for one V2 protocol request/response pair."""

WorkerArtifactIdV2: TypeAlias = Annotated[str, StringConstraints(min_length=1)]
"""Stable identifier for one prepared worker artifact."""

WorkerArtifactPathV2: TypeAlias = Annotated[str, StringConstraints(min_length=1)]
"""Filesystem path to a prepared worker artifact."""

ProtocolVersionV2: TypeAlias = Annotated[int, Field(ge=2)]
"""Worker protocol major version."""

FiniteNonNegativeFloat: TypeAlias = Annotated[FiniteFloat, Field(ge=0)]
"""Finite floating-point value constrained to be non-negative."""


class WorkerKindV2(str, Enum):
    """Worker role selected during the V2 handshake."""

    INFER = "infer"


class InferenceTaskV2(str, Enum):
    """High-level V2 task family."""

    MORPHOSYNTAX = "morphosyntax"
    UTSEG = "utseg"
    TRANSLATE = "translate"
    COREF = "coref"
    ASR = "asr"
    FORCED_ALIGNMENT = "forced_alignment"
    SPEAKER = "speaker"
    OPENSMILE = "opensmile"
    AVQI = "avqi"


class AsrBackendV2(str, Enum):
    """ASR backend selected by Rust."""

    LOCAL_WHISPER = "local_whisper"
    # HuggingFace Whisper fine-tune selected by model_id. Same worker-side
    # runtime shape as LOCAL_WHISPER — both host a ``WhisperASRHandle`` —
    # but a distinct backend variant so the control-plane pool key and the
    # worker's engine dispatch select the fine-tune loader at bootstrap.
    WHISPER_HUB = "whisper_hub"
    HK_TENCENT = "hk_tencent"
    HK_ALIYUN = "hk_aliyun"
    HK_FUNAUDIO = "hk_funaudio"
    REVAI = "revai"


class FaBackendV2(str, Enum):
    """Forced-alignment backend selected by Rust."""

    WHISPER = "whisper"
    WAVE2VEC = "wave2vec"
    WAV2VEC_CANTO = "wav2vec_canto"


class SpeakerBackendV2(str, Enum):
    """Speaker backend selected by Rust."""

    PYANNOTE = "pyannote"
    NEMO = "nemo"


class WorkerAttachmentKindV2(str, Enum):
    """Small attachment vocabulary advertised in capabilities."""

    PREPARED_AUDIO = "prepared_audio"
    PREPARED_TEXT = "prepared_text"
    INLINE_JSON = "inline_json"
    PROVIDER_MEDIA = "provider_media"
    SUBMITTED_JOB = "submitted_job"


class PreparedAudioEncodingV2(str, Enum):
    """PCM encoding used for prepared audio artifacts."""

    PCM_F32LE = "pcm_f32le"


class PreparedTextEncodingV2(str, Enum):
    """Encoding used for prepared text artifacts."""

    UTF8_JSON = "utf8_json"


class ProtocolErrorCodeV2(str, Enum):
    """Error category for protocol-level failures."""

    UNSUPPORTED_PROTOCOL = "unsupported_protocol"
    INVALID_PAYLOAD = "invalid_payload"
    MISSING_ATTACHMENT = "missing_attachment"
    ATTACHMENT_UNREADABLE = "attachment_unreadable"
    MODEL_UNAVAILABLE = "model_unavailable"
    RUNTIME_FAILURE = "runtime_failure"


class FaTextModeV2(str, Enum):
    """Text-joining mode for forced-alignment payloads."""

    SPACE_JOINED = "space_joined"
    CHAR_JOINED = "char_joined"


class WorkerRuntimeInfoV2(BaseModel):
    """Runtime information returned during the V2 handshake."""

    python_version: str
    free_threaded: bool


class HelloRequestV2(BaseModel):
    """Initial handshake request sent by Rust."""

    protocol_version: ProtocolVersionV2
    worker_kind: WorkerKindV2


class HelloResponseV2(BaseModel):
    """Initial handshake response sent by Python."""

    protocol_version: ProtocolVersionV2
    worker_pid: int
    runtime: WorkerRuntimeInfoV2


class CapabilitiesRequestV2(BaseModel):
    """Request for task capability metadata."""

    request_id: WorkerRequestIdV2


class TaskCapabilityV2(BaseModel):
    """One task capability advertised by a V2 worker."""

    task: InferenceTaskV2
    accepted_inputs: list[WorkerAttachmentKindV2]
    supports_progress_events: bool


class CapabilitiesResponseV2(BaseModel):
    """Response describing task capabilities for the worker."""

    request_id: WorkerRequestIdV2
    tasks: list[TaskCapabilityV2]
    engine_versions: dict[str, str]


class PreparedAudioRefV2(BaseModel):
    """File-backed prepared audio artifact."""

    kind: Literal["prepared_audio"] = "prepared_audio"
    id: WorkerArtifactIdV2
    path: WorkerArtifactPathV2
    encoding: PreparedAudioEncodingV2
    channels: int = Field(ge=1)
    sample_rate_hz: int = Field(ge=1)
    frame_count: int = Field(ge=0)
    byte_offset: int = Field(ge=0)
    byte_len: int = Field(ge=0)


class PreparedTextRefV2(BaseModel):
    """File-backed prepared text artifact."""

    kind: Literal["prepared_text"] = "prepared_text"
    id: WorkerArtifactIdV2
    path: WorkerArtifactPathV2
    encoding: PreparedTextEncodingV2
    byte_offset: int = Field(ge=0)
    byte_len: int = Field(ge=0)


class InlineJsonRefV2(BaseModel):
    """Small inline JSON attachment."""

    kind: Literal["inline_json"] = "inline_json"
    id: WorkerArtifactIdV2
    value: WorkerJSONValue


ArtifactRefV2: TypeAlias = Annotated[
    PreparedAudioRefV2 | PreparedTextRefV2 | InlineJsonRefV2,
    Field(discriminator="kind"),
]
"""Prepared artifact descriptor carried alongside an execute request."""


class PreparedAudioInputV2(BaseModel):
    """Reference to a prepared audio attachment (internally tagged)."""

    kind: Literal["prepared_audio"] = "prepared_audio"
    audio_ref_id: WorkerArtifactIdV2


class ProviderMediaInputV2(BaseModel):
    """Temporary cloud-provider media input (internally tagged)."""

    kind: Literal["provider_media"] = "provider_media"
    media_path: WorkerArtifactPathV2
    num_speakers: NumSpeakers


class SubmittedJobInputV2(BaseModel):
    """Previously submitted provider job id (internally tagged)."""

    kind: Literal["submitted_job"] = "submitted_job"
    provider_job_id: WorkerArtifactIdV2


# Backward-compatible aliases for ASR input wrappers.
PreparedAudioAsrInputV2 = PreparedAudioInputV2
ProviderMediaAsrInputV2 = ProviderMediaInputV2
SubmittedJobAsrInputV2 = SubmittedJobInputV2

AsrInputV2: TypeAlias = Annotated[
    PreparedAudioInputV2 | ProviderMediaInputV2 | SubmittedJobInputV2,
    Field(discriminator="kind"),
]
"""Backend-specific ASR input transport (internally tagged on ``kind``)."""


class AsrRequestV2(BaseModel):
    """V2 ASR request payload (internally tagged as ``"asr"``)."""

    kind: Literal["asr"] = "asr"
    lang: LanguageCode
    backend: AsrBackendV2
    input: AsrInputV2


class ForcedAlignmentRequestV2(BaseModel):
    """V2 forced-alignment request payload (internally tagged as ``"forced_alignment"``)."""

    kind: Literal["forced_alignment"] = "forced_alignment"
    backend: FaBackendV2
    payload_ref_id: WorkerArtifactIdV2
    audio_ref_id: WorkerArtifactIdV2
    text_mode: FaTextModeV2
    pauses: bool


class MorphosyntaxRequestV2(BaseModel):
    """V2 morphosyntax request payload (internally tagged as ``"morphosyntax"``)."""

    kind: Literal["morphosyntax"] = "morphosyntax"
    lang: LanguageCode
    payload_ref_id: WorkerArtifactIdV2
    item_count: int = Field(ge=0)
    retokenize: bool = False


class UtsegRequestV2(BaseModel):
    """V2 utterance-segmentation request payload (internally tagged as ``"utseg"``)."""

    kind: Literal["utseg"] = "utseg"
    lang: LanguageCode
    payload_ref_id: WorkerArtifactIdV2
    item_count: int = Field(ge=0)
    # Operator opt-in to the legacy Stanza constituency-parser
    # fallback for unsupported languages. Surfaced as
    # `--utseg-fallback-stanza` on the CLI. Defaults to `False` so a
    # request from an older client that omits the field deserializes
    # with the safe "refuse" behavior; replaces the previous
    # `BA3_UTSEG_FALLBACK_STANZA` env-var control flow.
    allow_stanza_fallback: bool = False


class TranslateRequestV2(BaseModel):
    """V2 translation request payload (internally tagged as ``"translate"``)."""

    kind: Literal["translate"] = "translate"
    source_lang: LanguageCode
    target_lang: LanguageCode
    payload_ref_id: WorkerArtifactIdV2
    item_count: int = Field(ge=0)


class CorefRequestV2(BaseModel):
    """V2 coreference request payload (internally tagged as ``"coref"``)."""

    kind: Literal["coref"] = "coref"
    lang: LanguageCode
    payload_ref_id: WorkerArtifactIdV2
    item_count: int = Field(ge=0)


class SpeakerRequestV2(BaseModel):
    """V2 speaker diarization request payload (internally tagged as ``"speaker"``)."""

    kind: Literal["speaker"] = "speaker"
    backend: SpeakerBackendV2
    input: "SpeakerInputV2"
    expected_speakers: NumSpeakers | None = None


class OpenSmileRequestV2(BaseModel):
    """V2 openSMILE request payload (internally tagged as ``"opensmile"``)."""

    kind: Literal["opensmile"] = "opensmile"
    audio_ref_id: WorkerArtifactIdV2
    feature_set: str = "eGeMAPSv02"
    feature_level: str = "functionals"


class AvqiRequestV2(BaseModel):
    """V2 AVQI request payload (internally tagged as ``"avqi"``)."""

    kind: Literal["avqi"] = "avqi"
    cs_audio_ref_id: WorkerArtifactIdV2
    sv_audio_ref_id: WorkerArtifactIdV2


class SpeakerPreparedAudioInputV2(BaseModel):
    """Prepared-audio speaker input owned by Rust (internally tagged)."""

    kind: Literal["prepared_audio"] = "prepared_audio"
    audio_ref_id: WorkerArtifactIdV2


# Backward-compatible alias.
SpeakerPreparedAudioRefInputV2 = SpeakerPreparedAudioInputV2

SpeakerInputV2: TypeAlias = Annotated[
    SpeakerPreparedAudioInputV2,
    Field(discriminator="kind"),
]
"""Speaker input transport (internally tagged on ``kind``)."""


# Backward-compatible aliases: with internally tagged unions, the inner
# request structs carry the ``kind`` field directly — no wrapper needed.
AsrTaskRequestV2 = AsrRequestV2
ForcedAlignmentTaskRequestV2 = ForcedAlignmentRequestV2
MorphosyntaxTaskRequestV2 = MorphosyntaxRequestV2
UtsegTaskRequestV2 = UtsegRequestV2
TranslateTaskRequestV2 = TranslateRequestV2
CorefTaskRequestV2 = CorefRequestV2
SpeakerTaskRequestV2 = SpeakerRequestV2
OpenSmileTaskRequestV2 = OpenSmileRequestV2
AvqiTaskRequestV2 = AvqiRequestV2


TaskRequestV2: TypeAlias = Annotated[
    AsrRequestV2
    | ForcedAlignmentRequestV2
    | MorphosyntaxRequestV2
    | UtsegRequestV2
    | TranslateRequestV2
    | CorefRequestV2
    | SpeakerRequestV2
    | OpenSmileRequestV2
    | AvqiRequestV2,
    Field(discriminator="kind"),
]
"""Typed execute request payload (internally tagged on ``kind``)."""


class ExecuteRequestV2(BaseModel):
    """One top-level V2 execution request."""

    request_id: WorkerRequestIdV2
    task: InferenceTaskV2
    payload: TaskRequestV2
    attachments: list[ArtifactRefV2]


class WhisperChunkSpanV2(BaseModel):
    """One raw Whisper chunk span returned by Python."""

    text: str
    start_s: FiniteNonNegativeFloat
    end_s: FiniteNonNegativeFloat

    @model_validator(mode="after")
    def _validate_range(self) -> WhisperChunkSpanV2:
        if self.end_s < self.start_s:
            raise ValueError("Whisper chunk end_s must be >= start_s")
        return self


class WhisperChunkResultPayloadV2(BaseModel):
    """Raw Whisper chunk output returned by Python (internally tagged)."""

    kind: Literal["whisper_chunk_result"] = "whisper_chunk_result"
    lang: LanguageCode
    text: str
    chunks: list[WhisperChunkSpanV2]


class AsrElementKindV2(str, Enum):
    """Stable vocabulary for one monologue element returned by ASR."""

    TEXT = "text"
    PUNCTUATION = "punctuation"


class AsrElementV2(BaseModel):
    """One raw ASR element inside a speaker monologue."""

    value: str
    start_s: FiniteNonNegativeFloat | None = None
    end_s: FiniteNonNegativeFloat | None = None
    kind: AsrElementKindV2
    confidence: FiniteFloat | None = None

    @model_validator(mode="after")
    def _validate_range(self) -> AsrElementV2:
        if self.start_s is not None and self.end_s is not None and self.end_s < self.start_s:
            raise ValueError("ASR element end_s must be >= start_s")
        return self


class AsrMonologueV2(BaseModel):
    """One speaker-attributed monologue returned by a provider backend."""

    speaker: SpeakerId
    elements: list[AsrElementV2]


class MonologueAsrResultPayloadV2(BaseModel):
    """Provider-shaped ASR output returned as speaker monologues (internally tagged)."""

    kind: Literal["monologue_asr_result"] = "monologue_asr_result"
    lang: LanguageCode
    monologues: list[AsrMonologueV2]


class WhisperTokenTimingV2(BaseModel):
    """One raw Whisper forced-alignment token onset."""

    text: str
    time_s: FiniteNonNegativeFloat


class WhisperTokenTimingResultPayloadV2(BaseModel):
    """Raw Whisper forced-alignment token output (internally tagged)."""

    kind: Literal["whisper_token_timing_result"] = "whisper_token_timing_result"
    tokens: list[WhisperTokenTimingV2]


class IndexedWordTimingV2(BaseModel):
    """One word-level timing result."""

    start_ms: int = Field(ge=0)
    end_ms: int = Field(ge=0)
    confidence: FiniteFloat | None = None

    @model_validator(mode="after")
    def _validate_range(self) -> IndexedWordTimingV2:
        if self.end_ms < self.start_ms:
            raise ValueError("Indexed word timing end_ms must be >= start_ms")
        return self


class IndexedWordTimingResultPayloadV2(BaseModel):
    """Forced-alignment indexed timing output (internally tagged)."""

    kind: Literal["indexed_word_timing_result"] = "indexed_word_timing_result"
    indexed_timings: list[IndexedWordTimingV2 | None]


class MorphosyntaxItemResultV2(BaseModel):
    """One morphosyntax item result returned by Python."""

    raw_sentences: list[WorkerJSONValue] | None = None
    error: str | None = None


class MorphosyntaxResultPayloadV2(BaseModel):
    """Batched morphosyntax response payload (internally tagged)."""

    kind: Literal["morphosyntax_result"] = "morphosyntax_result"
    items: list[MorphosyntaxItemResultV2]


class UtsegItemResultV2(BaseModel):
    """One utterance-segmentation item result returned by Python."""

    assignments: list[int] | None = None
    trees: list[str] | None = None
    error: str | None = None


class UtsegResultPayloadV2(BaseModel):
    """Batched utterance-segmentation response payload (internally tagged)."""

    kind: Literal["utseg_result"] = "utseg_result"
    items: list[UtsegItemResultV2]


class TranslationItemResultV2(BaseModel):
    """One translation item result returned by Python."""

    raw_translation: str | None = None
    error: str | None = None


class TranslationResultPayloadV2(BaseModel):
    """Batched translation response payload (internally tagged)."""

    kind: Literal["translation_result"] = "translation_result"
    items: list[TranslationItemResultV2]


class CorefChainRefV2(BaseModel):
    """One structured coreference chain reference returned by Python."""

    chain_id: int = Field(ge=0)
    is_start: bool
    is_end: bool


class CorefAnnotationV2(BaseModel):
    """One per-sentence coreference annotation returned by Python."""

    sentence_idx: int = Field(ge=0)
    words: list[list[CorefChainRefV2]]


class CorefItemResultV2(BaseModel):
    """One coreference item result returned by Python."""

    annotations: list[CorefAnnotationV2] | None = None
    error: str | None = None


class CorefResultPayloadV2(BaseModel):
    """Batched coreference response payload (internally tagged)."""

    kind: Literal["coref_result"] = "coref_result"
    items: list[CorefItemResultV2]


class SpeakerSegmentV2(BaseModel):
    """One raw speaker diarization segment returned by Python."""

    start_ms: int = Field(ge=0)
    end_ms: int = Field(ge=0)
    speaker: SpeakerId

    @model_validator(mode="after")
    def _validate_range(self) -> SpeakerSegmentV2:
        if self.end_ms < self.start_ms:
            raise ValueError("Speaker segment end_ms must be >= start_ms")
        return self


class SpeakerResultPayloadV2(BaseModel):
    """Raw speaker diarization output returned by the model host (internally tagged)."""

    kind: Literal["speaker_result"] = "speaker_result"
    segments: list[SpeakerSegmentV2]


class OpenSmileResultPayloadV2(BaseModel):
    """Raw openSMILE tabular output returned by the model host (internally tagged)."""

    kind: Literal["opensmile_result"] = "opensmile_result"
    feature_set: str
    feature_level: str
    num_features: int = Field(ge=0)
    duration_segments: int = Field(ge=0)
    audio_file: str
    rows: list[dict[str, FiniteFloat]]
    success: bool
    error: str | None = None


class AvqiResultPayloadV2(BaseModel):
    """Raw AVQI metrics returned by the model host (internally tagged)."""

    kind: Literal["avqi_result"] = "avqi_result"
    avqi: FiniteFloat
    cpps: FiniteFloat
    hnr: FiniteFloat
    shimmer_local: FiniteFloat
    shimmer_local_db: FiniteFloat
    slope: FiniteFloat
    tilt: FiniteFloat
    cs_file: str
    sv_file: str
    success: bool
    error: str | None = None


# Backward-compatible aliases: with internally tagged unions, the payload
# structs carry the ``kind`` field directly — no wrapper needed.
WhisperChunkResultV2 = WhisperChunkResultPayloadV2
MonologueAsrResultV2 = MonologueAsrResultPayloadV2
WhisperTokenTimingResultV2 = WhisperTokenTimingResultPayloadV2
IndexedWordTimingResultV2 = IndexedWordTimingResultPayloadV2
MorphosyntaxResultV2 = MorphosyntaxResultPayloadV2
UtsegResultV2 = UtsegResultPayloadV2
TranslationResultV2 = TranslationResultPayloadV2
CorefResultV2 = CorefResultPayloadV2
SpeakerResultV2 = SpeakerResultPayloadV2
OpenSmileResultV2 = OpenSmileResultPayloadV2
AvqiResultV2 = AvqiResultPayloadV2

TaskResultV2: TypeAlias = Annotated[
    WhisperChunkResultPayloadV2
    | MonologueAsrResultPayloadV2
    | WhisperTokenTimingResultPayloadV2
    | IndexedWordTimingResultPayloadV2
    | MorphosyntaxResultPayloadV2
    | UtsegResultPayloadV2
    | TranslationResultPayloadV2
    | CorefResultPayloadV2
    | SpeakerResultPayloadV2
    | OpenSmileResultPayloadV2
    | AvqiResultPayloadV2,
    Field(discriminator="kind"),
]
"""Typed execute result payload (internally tagged on ``kind``)."""


class ExecuteSuccessV2(BaseModel):
    """Successful execute outcome."""

    kind: Literal["success"] = "success"


class ExecuteErrorV2(BaseModel):
    """Protocol/runtime failure outcome."""

    kind: Literal["error"] = "error"
    code: ProtocolErrorCodeV2
    message: str


ExecuteOutcomeV2: TypeAlias = Annotated[
    ExecuteSuccessV2 | ExecuteErrorV2,
    Field(discriminator="kind"),
]
"""Top-level execute outcome."""


class ExecuteResponseV2(BaseModel):
    """Top-level V2 execute response."""

    request_id: WorkerRequestIdV2
    outcome: ExecuteOutcomeV2
    result: TaskResultV2 | None = None
    elapsed_s: FiniteNonNegativeFloat


class ProgressEventV2(BaseModel):
    """Progress event emitted by long-running V2 tasks."""

    request_id: WorkerRequestIdV2
    completed: int = Field(ge=0)
    total: int = Field(ge=0)
    stage: str


class ShutdownRequestV2(BaseModel):
    """Shutdown request sent to a V2 worker."""

    request_id: WorkerRequestIdV2
