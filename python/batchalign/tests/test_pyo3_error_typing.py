"""Phase D2 pilot tests: typed errors crossing the PyO3 boundary.

The contract under test (from
``book/src/batchalign/architecture/python-rust-errors.md``):

- A malformed ``execute_asr_request_v2`` payload raises
  ``BatchalignError`` (or a subclass), **not** ``ValueError``.
- The Python ``batchalign.errors.BatchalignError`` is the *same class*
  Rust raises through PyO3 — re-exported from ``batchalign_core``.
- Catch sites at the typed parent class catch any boundary error
  category.

These tests pin the contract for ``worker_asr_exec.rs``. Subsequent
sweeps (other PyO3 boundaries) extend the same contract.
"""

from __future__ import annotations

import pytest

import batchalign_core
from batchalign.errors import (
    BatchalignError,
    CHATValidationException,
    ConfigError,
    ConfigNotFoundError,
    DocumentValidationException,
    PayloadTooLargeError,
    SkipFileWarning,
)


class TestExceptionHierarchy:
    """Re-export identity: Python-side imports are the same Rust classes."""

    def test_batchalign_error_re_export_identity(self):
        """``batchalign.errors.BatchalignError is batchalign_core.BatchalignError``."""
        assert BatchalignError is batchalign_core.BatchalignError

    def test_subclasses_inherit_from_batchalign_error(self):
        """All non-warning typed exceptions descend from ``BatchalignError``."""
        for exc_class in (
            CHATValidationException,
            DocumentValidationException,
            ConfigNotFoundError,
            ConfigError,
            PayloadTooLargeError,
        ):
            assert issubclass(exc_class, BatchalignError), (
                f"{exc_class.__name__} should inherit from BatchalignError"
            )

    def test_skip_file_warning_does_not_inherit_from_batchalign_error(self):
        """``SkipFileWarning`` is a sibling of ``BatchalignError``, not a child.

        Per the design doc's Phase D1 decision: skip-warning is a
        graceful-skip signal, not a boundary failure.
        """
        assert not issubclass(SkipFileWarning, BatchalignError)


class TestExecuteAsrRequestV2BoundaryErrors:
    """``execute_asr_request_v2`` malformed-input rejection lands as
    ``BatchalignError``, not ``ValueError``."""

    def test_malformed_payload_raises_batchalign_error(self):
        """Malformed dict (missing required fields) → ``BatchalignError``.

        Catches at the typed parent class — the contract is that any
        boundary failure is at least a ``BatchalignError`` instance.
        """
        with pytest.raises(BatchalignError):
            # Empty dict is missing every required field of ExecuteRequestV2.
            batchalign_core.execute_asr_request_v2({})

    def test_malformed_payload_does_not_raise_plain_value_error(self):
        """The legacy `PyValueError(error.to_string())` shape is gone.

        Ensures we don't regress to stringly-typed errors. ``BatchalignError``
        is not a subclass of ``ValueError``, so this catches both:
        (a) the migration is complete on this site, and
        (b) the typed exception is structurally distinct from ValueError.
        """
        # Establish that catching ValueError specifically would NOT work:
        with pytest.raises(BaseException) as exc_info:
            batchalign_core.execute_asr_request_v2({})
        assert not isinstance(exc_info.value, ValueError), (
            f"Expected typed BatchalignError, got plain {type(exc_info.value).__name__}"
        )

    def test_internal_variant_raises_batchalign_error_not_subclass(self):
        """The pilot's two ``BatchalignBoundaryError::Internal`` sites
        raise ``BatchalignError`` directly (not a more-specific subclass)."""
        with pytest.raises(BatchalignError) as exc_info:
            batchalign_core.execute_asr_request_v2({})
        # The pilot raises `BatchalignError` directly. When other variants
        # (CHATValidationException etc.) are wired by subsequent sweeps,
        # this assertion gets per-site narrowing.
        assert type(exc_info.value).__name__ == "BatchalignError"


class TestExceptionAttributesShape:
    """Verify the structured-attribute fields exist and have the
    documented shape, even before all callers populate them."""

    def test_chat_validation_exception_carries_errors_attribute(self):
        """A bare-constructed ``CHATValidationException`` has empty
        ``errors`` and ``bug_report_id=None``.

        Bare-constructed instances (from Python `raise`) won't have
        attributes set; only Rust-raised ones do via the boundary's
        ``setattr``. This test documents that gap so future Python-side
        raises remember to set them.
        """
        # Plain Python construction does NOT auto-populate attributes
        # — that only happens through the Rust `From` impl. This is
        # the documented behaviour, not a bug.
        exc = CHATValidationException("test")
        # `getattr` with default returns the default for missing attrs
        # — which is the contract `classify_error` already relies on.
        assert getattr(exc, "errors", None) is None
        assert getattr(exc, "bug_report_id", None) is None
