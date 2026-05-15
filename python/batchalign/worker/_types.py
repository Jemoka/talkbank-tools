"""Request/Response models and worker state (V1 protocol).

These mirror Rust batchalign-types::worker and are the wire format
for the stdio JSON-lines IPC protocol between the Rust server and
stateless Python inference workers.

V1 Protocol Status: FROZEN
~~~~~~~~~~~~~~~~~~~~~~~~~
V1 is used for morphosyntax, utseg, translate, and coref batch inference.
All new task families (FA, ASR, speaker, opensmile, avqi) use V2
(see ``_types_v2.py``). V1 types are not part of the Rust→Python schema
generation pipeline and are not covered by conformance tests.

Do not add new types to V1. New engines and commands should use V2.
"""

from __future__ import annotations

import threading
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass, field
import time
from enum import Enum
from typing import TYPE_CHECKING
from typing_extensions import TypeAliasType

from pydantic import BaseModel, Field

from batchalign.device import DevicePolicy
from batchalign.inference._domain_types import (
    CommandName,
    LanguageCode,
    NumSpeakers,
    RevAiApiKey,
    TimestampMs,
    TranslationBackend,
)

if TYPE_CHECKING:
    from batchalign.inference._tokenizer_realign import TokenizerContext
    from batchalign.inference.types import (
        StanzaNLP,
        Wave2VecFAHandle,
        WhisperASRHandle,
        WhisperFAHandle,
    )
    from batchalign.models.utterance.infer import BertUtteranceModel

JSONPrimitive = str | int | float | bool | None

if TYPE_CHECKING:
    WorkerJSONValue = JSONPrimitive | Sequence["WorkerJSONValue"] | Mapping[str, "WorkerJSONValue"]
else:
    WorkerJSONValue = TypeAliasType(
        "WorkerJSONValue",
        JSONPrimitive | Sequence["WorkerJSONValue"] | Mapping[str, "WorkerJSONValue"],
    )


class HealthResponse(BaseModel):
    """Response body for health operation."""

    status: str = "ok"
    command: CommandName = ""
    lang: LanguageCode = ""
    pid: int = 0
    uptime_s: float = 0.0


class InferTask(str, Enum):
    """Supported inference task identifiers (snake_case on the wire)."""

    MORPHOSYNTAX = "morphosyntax"
    UTSEG = "utseg"
    TRANSLATE = "translate"
    COREF = "coref"
    FA = "fa"
    ASR = "asr"
    OPENSMILE = "opensmile"
    AVQI = "avqi"
    SPEAKER = "speaker"


class WorkerProfile(str, Enum):
    """Worker profile grouping related InferTasks into fewer processes.

    Instead of spawning one worker per InferTask, profiles group related tasks
    to share loaded models within a single process:

    - GPU: ASR, FA, Speaker — GPU-bound models, concurrent via ThreadPoolExecutor
    - STANZA: Morphosyntax, Utseg, Coref — Stanza NLP processors
    - IO: Translate, OpenSMILE, AVQI — lightweight API/library calls
    """

    GPU = "gpu"
    STANZA = "stanza"
    IO = "io"


PROFILE_TASKS: dict[WorkerProfile, set[str]] = {
    WorkerProfile.GPU: {"asr", "fa", "speaker"},
    WorkerProfile.STANZA: {"morphosyntax", "utseg", "coref"},
    WorkerProfile.IO: {"translate", "opensmile", "avqi"},
}


@dataclass(frozen=True, slots=True)
class WorkerBootstrapRuntime:
    """Typed worker bootstrap inputs resolved once at process startup."""

    task: InferTask | None
    lang: LanguageCode
    num_speakers: NumSpeakers
    profile: WorkerProfile | None = None
    engine_overrides: dict[str, str] = field(default_factory=dict)
    test_echo: bool = False
    verbose: int = 0
    device_policy: DevicePolicy = field(default_factory=DevicePolicy)
    revai_api_key: RevAiApiKey | None = None


class AsrEngine(str, Enum):
    """Which ASR backend is loaded for this worker."""

    WHISPER = "whisper"
    WHISPER_HUB = "whisper_hub"
    REV = "rev"
    TENCENT = "tencent"
    ALIYUN = "aliyun"
    FUNAUDIO = "funaudio"


class FaEngine(str, Enum):
    """Which forced-alignment backend is loaded for this worker."""

    WHISPER = "whisper"
    WAVE2VEC = "wave2vec"
    WAV2VEC_CANTO = "wav2vec_canto"


class StanzaLanguageProcessors(BaseModel):
    """Per-language Stanza processor availability."""

    alpha2: str
    processors: list[str]


class CapabilitiesResponse(BaseModel):
    """Response body for capabilities operation."""

    commands: list[str]
    free_threaded: bool
    infer_tasks: list[InferTask]
    engine_versions: dict[InferTask, str]
    stanza_capabilities: dict[str, StanzaLanguageProcessors] = Field(
        default_factory=dict
    )


class InferRequest(BaseModel):
    """Request body for infer operation -- pure inference, no CHAT."""

    task: InferTask
    lang: LanguageCode
    payload: WorkerJSONValue = Field(default_factory=dict)


class InferResponse(BaseModel):
    """Response body for infer operation."""

    result: WorkerJSONValue | None = None
    error: str | None = None
    elapsed_s: float = 0.0


class BatchInferRequest(BaseModel):
    """Request body for batch_infer operation -- multiple inference items."""

    task: InferTask
    lang: LanguageCode
    items: list[WorkerJSONValue] = Field(default_factory=list)
    mwt: dict[str, list[str]] = Field(default_factory=dict)
    retokenize: bool = False
    # Operator opt-in to the legacy Stanza constituency-parser fallback
    # for utseg when no language-specific TalkBank BERT model is
    # configured. Surfaced as `--utseg-fallback-stanza` on every
    # utseg-invoking CLI subcommand (transcribe, transcribe_s, utseg).
    # Default-refuse mirrors the WhisperHubModelNotFoundError pattern:
    # silent substitution between models is the foot-gun this flag
    # exists to prevent.
    allow_stanza_fallback: bool = False


class BatchInferResponse(BaseModel):
    """Response body for batch_infer operation."""

    results: list[InferResponse]


BatchInferHandler = Callable[[BatchInferRequest], BatchInferResponse]
"""Callable signature for one fully wired batch-infer task handler."""


# ---------------------------------------------------------------------------
# Application state
# ---------------------------------------------------------------------------


class _WorkerState:
    """Mutable state for the worker process.

    Model objects are loaded directly at worker startup. The stanza_*
    fields hold loaded Stanza pipelines for morphosyntax/utseg inference.
    The fa_* fields hold loaded forced alignment models.
    """

    def __init__(self) -> None:
        self.command: CommandName = ""
        self.lang: LanguageCode = ""
        self.num_speakers: NumSpeakers = 1
        self.started_at: float = time.monotonic()
        self.test_echo: bool = False
        self.test_delay_ms: TimestampMs = 0
        self.ready: bool = False
        self.bootstrap: WorkerBootstrapRuntime | None = None

        # Stanza models for morphosyntax
        self.stanza_pipelines: dict[LanguageCode, StanzaNLP] | None = None
        self.stanza_contexts: dict[LanguageCode, TokenizerContext] | None = None
        self.stanza_nlp_lock: threading.Lock | None = None
        self.stanza_version: str = ""

        # Utseg config builder (callable from StanzaUtteranceEngine)
        self.utseg_config_builder: Callable[
            [list[str]], tuple[list[str], dict[str, dict[str, str | bool]]]
        ] | None = None
        self.utterance_boundary_model: BertUtteranceModel | None = None
        self.utterance_model_name: str = ""
        self.utseg_version: str = ""

        # Translation
        self.translate_backend: TranslationBackend | None = None
        self.translate_fn: Callable[[str, str], str] | None = None

        # FA models (typed handles from load_whisper_fa / load_wave2vec_fa)
        self.whisper_fa_model: WhisperFAHandle | None = None
        self.wave2vec_fa_model: Wave2VecFAHandle | None = None
        self.fa_model_name: str = ""

        # ASR model
        self.whisper_asr_model: WhisperASRHandle | None = None
        self.rev_api_key: RevAiApiKey | None = None
        self.asr_engine: AsrEngine = AsrEngine.WHISPER

        # FA engine tracking
        self.fa_engine: FaEngine = FaEngine.WHISPER

        # Tracks which tasks have been loaded (for LazyProfile mode).
        # Empty at startup in lazy mode; populated as ensure_task_loaded() is
        # called. In eager (Profile/Task) mode, populated during bootstrap.
        self.loaded_tasks: set[str] = set()

        # Serializes on-demand model loading in LazyProfile mode. Prevents
        # concurrent ensure_task calls from loading the same model twice.
        self.loading_lock: threading.Lock = threading.Lock()

        # Request-time batch inference handlers registered during bootstrap.
        # Dynamic tasks such as translation, FA, and ASR should resolve their
        # engine-specific routing here instead of branching on raw state in the
        # hot request path.
        self.batch_infer_handlers: dict[InferTask, BatchInferHandler] = {}

        # Transient progress callback set by the V2 execution path during a
        # long-running morphosyntax batch.  Read by the handler to emit
        # ProgressEventV2 lines.  None when no V2 request is in flight.
        self.active_progress_callback: Callable[[int, int], None] | None = None

    def register_batch_infer_handler(
        self,
        task: InferTask,
        handler: BatchInferHandler,
    ) -> None:
        """Install the concrete batch-infer handler for one loaded task."""
        self.batch_infer_handlers[task] = handler

    def batch_infer_handler(
        self,
        task: InferTask,
    ) -> BatchInferHandler | None:
        """Return the bootstrap-installed batch handler for one task."""
        return self.batch_infer_handlers.get(task)

    def clear_batch_infer_handlers(self) -> None:
        """Clear any previously registered task handlers.

        Worker startup is a one-command bootstrap boundary. Clearing the
        registry before reconfiguration keeps test setup and future worker
        reinitialization paths from accidentally reusing stale engine wiring.
        """
        self.batch_infer_handlers.clear()


_state = _WorkerState()
