"""Batchalign exception hierarchy.

The exception classes are defined in the Rust ``batchalign_core`` PyO3
module so the Python and Rust sides see the same types: an exception
raised from Rust at the PyO3 boundary is the *same Python class* a
Python catch site sees.  See
``book/src/batchalign/architecture/python-rust-errors.md`` for the
full contract.

Error Hierarchy
---------------
::

    Exception
    +-- BatchalignError                     # Common ancestor for boundary failures
    |   +-- CHATValidationException         # Structural CHAT problems (parse or validation)
    |   +-- DocumentValidationException     # Non-CHAT document payload problems
    |   +-- ConfigNotFoundError             # Required config file missing on disk
    |   +-- ConfigError                     # Config present but semantically invalid
    |   +-- PayloadTooLargeError            # HTTP body limit rejection
    +-- SkipFileWarning                     # Graceful skip (file passes through unchanged)

The :func:`classify_error` helper maps arbitrary exceptions to one of
five user-facing categories (``validation``, ``input``, ``media``,
``system``, ``processing``) for CLI progress display and server job
status reporting.

Attributes set on Rust-raised exceptions
----------------------------------------
``CHATValidationException``: ``errors`` (list[ValidationErrorEntry]),
``bug_report_id`` (str | None).

``ConfigNotFoundError``: ``path`` (str).

``PayloadTooLargeError``: ``limit_layer`` (str), ``configured_bytes``
(int).

``SkipFileWarning``: ``chat_text`` (str | None).
"""

from typing import TypedDict

from batchalign_core import (
    BatchalignError,
    CHATValidationException,
    ConfigError,
    ConfigNotFoundError,
    DocumentValidationException,
    PayloadTooLargeError,
    SkipFileWarning,
)

__all__ = [
    "BatchalignError",
    "CHATValidationException",
    "ConfigError",
    "ConfigNotFoundError",
    "DocumentValidationException",
    "PayloadTooLargeError",
    "SkipFileWarning",
    "ValidationErrorEntry",
    "classify_error",
]


class ValidationErrorEntry(TypedDict, total=False):
    """One structured validation error from ``validate_structured()``.

    Mirrors the Rust-side ``ValidationErrorEntry`` shape so attributes
    set by the PyO3 boundary on ``CHATValidationException.errors`` can
    be type-checked here too.
    """

    code: str
    severity: str
    line: int
    column: int
    message: str
    suggestion: str


def classify_error(exc: BaseException) -> str:
    """Classify an exception into a user-facing error category.

    Used by the CLI progress display and the server's job status
    reporting to assign a human-readable category to each per-file
    failure.

    Returns one of:

    - ``"validation"`` -- pipeline-produced validation bug (a
      ``CHATValidationException`` with a populated ``bug_report_id``).
    - ``"input"`` -- malformed CHAT input that the user should fix.
    - ``"media"`` -- missing audio/video file or filesystem path error.
    - ``"system"`` -- memory exhaustion or other infrastructure failure.
    - ``"processing"`` -- catch-all for all other processing failures.
    """
    if isinstance(exc, CHATValidationException):
        if getattr(exc, "bug_report_id", None) is not None:
            return "validation"
        return "input"

    if isinstance(exc, ValueError) and ("CHAT" in str(exc) or "Parse error" in str(exc)):
        return "input"

    if isinstance(exc, FileNotFoundError):
        return "media"

    if isinstance(exc, MemoryError):
        return "system"

    return "processing"
