"""Type stubs for the batchalign_core Rust worker runtime extension.

This stub exposes type information for mypy to check Python code
that calls into the Rust worker runtime.
"""

from collections.abc import Callable


# ---------------------------------------------------------------------------
# Worker protocol dispatch
# ---------------------------------------------------------------------------

def dispatch_protocol_message(
    message: object,
    *,
    health_fn: Callable[..., object],
    capabilities_fn: Callable[..., object],
    infer_fn: Callable[..., object],
    batch_infer_fn: Callable[..., object],
    execute_v2_fn: Callable[..., object],
    ensure_task_fn: Callable[..., object],
    infer_request_model: object,
    batch_infer_request_model: object,
    execute_v2_request_model: object,
    validation_error_type: object,
) -> tuple[dict[str, object], bool]: ...


# ---------------------------------------------------------------------------
# Worker V2 execution
# ---------------------------------------------------------------------------

def execute_asr_request_v2(
    request: object,
    local_whisper_runner: Callable[..., object] | None = None,
    hk_tencent_runner: Callable[..., object] | None = None,
    hk_aliyun_runner: Callable[..., object] | None = None,
    hk_funaudio_runner: Callable[..., object] | None = None,
) -> str: ...

def execute_forced_alignment_request_v2(
    request: object,
    whisper_runner: Callable[..., object] | None = None,
    wave2vec_runner: Callable[..., object] | None = None,
    canto_runner: Callable[..., object] | None = None,
) -> str: ...

def execute_opensmile_request_v2(
    request: object,
    prepared_audio_runner: Callable[..., object] | None = None,
) -> str: ...

def execute_avqi_request_v2(
    request: object,
    prepared_audio_runner: Callable[..., object] | None = None,
) -> str: ...

def execute_speaker_request_v2(
    request: object,
    pyannote_prepared_audio_runner: Callable[..., object] | None = None,
    nemo_prepared_audio_runner: Callable[..., object] | None = None,
) -> str: ...

def normalize_text_task_result(
    task: str,
    response: object,
    expected_count: int,
) -> str: ...

def align_tokens(
    original_words: list[str],
    stanza_tokens: list[str],
    alpha2: str = "",
) -> list[str | tuple[str, bool]]: ...


# ---------------------------------------------------------------------------
# Worker artifact loaders
# ---------------------------------------------------------------------------

def find_worker_attachment_by_id(
    attachments: object,
    artifact_id: str,
) -> str: ...

def load_worker_json_attachment(
    attachments: object,
    artifact_id: str,
) -> str: ...

def load_worker_prepared_text_json(
    attachment: object,
) -> str: ...

def load_worker_prepared_audio_f32le_bytes(
    attachment: object,
) -> bytes: ...


# ---------------------------------------------------------------------------
# Cantonese ASR bridges
# ---------------------------------------------------------------------------

def clean_funaudio_segment_text(text: str) -> str: ...

def funaudio_segments_to_asr(segments: object, lang: str) -> str: ...

def tencent_result_detail_to_asr(result_detail: object, lang: str) -> str: ...

def aliyun_sentences_to_asr(sentences: object, lang: str) -> str: ...

def normalize_cantonese(text: str) -> str: ...

def cantonese_char_tokens(text: str) -> list[str]: ...


# ---------------------------------------------------------------------------
# Typed Python exception hierarchy crossing the PyO3 boundary.
#
# Defined in ``crates/batchalign-pyo3/src/error.rs`` via
# ``pyo3::create_exception!``. ``BatchalignError`` is the common ancestor
# (inherits from ``Exception``); the subclasses below all inherit from
# ``BatchalignError``, except ``SkipFileWarning`` which inherits from
# ``Exception`` directly (it is raised and caught as an exception, not a
# warning — see error.rs comment).
# ---------------------------------------------------------------------------

class BatchalignError(Exception): ...
class CHATValidationException(BatchalignError): ...
class DocumentValidationException(BatchalignError): ...
class ConfigNotFoundError(BatchalignError): ...
class ConfigError(BatchalignError): ...
class PayloadTooLargeError(BatchalignError): ...
class SkipFileWarning(Exception): ...


