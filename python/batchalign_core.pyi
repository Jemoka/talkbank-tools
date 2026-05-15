"""Type stubs for the ``batchalign_core`` PyO3 module.

This stub covers the typed boundary exception classes introduced
2026-04-29 (Phase D2 pilot). The PyO3-exported execution functions
(``execute_asr_request_v2`` etc.) are intentionally not yet stubbed —
they will land alongside their respective Phase D3 sweep migrations.
See ``book/src/batchalign/architecture/python-rust-errors.md``.
"""

from __future__ import annotations

class BatchalignError(Exception):
    """Common ancestor for every typed exception raised across the
    Rust/Python PyO3 boundary."""

class CHATValidationException(BatchalignError):
    """Raised when CHAT validation detects structural problems."""

    errors: list[dict[str, object]]
    bug_report_id: str | None

class DocumentValidationException(BatchalignError):
    """Raised when validating a non-CHAT document payload fails."""

class ConfigNotFoundError(BatchalignError):
    """Raised when required Batchalign config files are missing."""

    path: str

class ConfigError(BatchalignError):
    """Raised for syntactically present but semantically invalid config."""

class PayloadTooLargeError(BatchalignError):
    """Raised when an HTTP request body exceeds the configured limit."""

    limit_layer: str
    configured_bytes: int

class SkipFileWarning(Exception):
    """Signals that a file should be skipped with a warning, not failed."""

    chat_text: str | None

# ── Functions (existing PyO3 entry points; minimally typed) ──
def execute_asr_request_v2(
    request: object,
    local_whisper_runner: object | None = ...,
    hk_tencent_runner: object | None = ...,
    hk_aliyun_runner: object | None = ...,
    hk_funaudio_runner: object | None = ...,
) -> str: ...
