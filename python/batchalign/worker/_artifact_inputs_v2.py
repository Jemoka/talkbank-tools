"""Thin readers for worker protocol V2 prepared artifacts.

The V2 worker boundary is intentionally split in two:

- a typed control plane with request/response envelopes
- a data plane with prepared artifact descriptors owned by Rust

This module implements the Python side of the second half. It should stay
minimal: read immutable prepared inputs that Rust already normalized, then
hand raw arrays or JSON objects to the model adapter. It must not grow into a
workflow or preprocessing layer.
"""

from __future__ import annotations

import json
from typing import TypeVar

import numpy as np
from pydantic import TypeAdapter

from batchalign.errors import BatchalignError
from batchalign.worker._types_v2 import (
    ArtifactRefV2,
    InlineJsonRefV2,
    PreparedAudioEncodingV2,
    PreparedAudioRefV2,
    PreparedTextEncodingV2,
    PreparedTextRefV2,
    WorkerArtifactIdV2,
)

_AttachmentT = TypeVar("_AttachmentT", PreparedAudioRefV2, PreparedTextRefV2, InlineJsonRefV2)
_ARTIFACT_REF_ADAPTER: TypeAdapter[ArtifactRefV2] = TypeAdapter(ArtifactRefV2)


class ArtifactInputErrorV2(ValueError):
    """Raised when a staged V2 prepared artifact is missing or malformed."""


def find_attachment_by_id_v2(
    attachments: list[ArtifactRefV2],
    artifact_id: WorkerArtifactIdV2,
) -> ArtifactRefV2:
    """Return the attachment with the requested id.

    The staged V2 contract keeps attachment lookup explicit so callers do not
    accidentally index into the envelope by position.
    """

    import batchalign_core

    try:
        raw = batchalign_core.find_worker_attachment_by_id(attachments, artifact_id)
    except (ValueError, TypeError, BatchalignError) as error:
        raise ArtifactInputErrorV2(str(error)) from error
    return _ARTIFACT_REF_ADAPTER.validate_json(raw)


def require_attachment_type_v2(
    attachments: list[ArtifactRefV2],
    artifact_id: WorkerArtifactIdV2,
    expected_type: type[_AttachmentT],
) -> _AttachmentT:
    """Return one attachment and assert it has the required descriptor type."""

    attachment = find_attachment_by_id_v2(attachments, artifact_id)
    if not isinstance(attachment, expected_type):
        raise ArtifactInputErrorV2(
            "worker protocol V2 attachment "
            f"{artifact_id!r} had type {type(attachment).__name__}, "
            f"expected {expected_type.__name__}"
        )
    return attachment


def load_json_attachment_v2(
    attachments: list[ArtifactRefV2],
    artifact_id: WorkerArtifactIdV2,
) -> object:
    """Load a JSON-bearing attachment by id.

    Callers should use this helper for prepared text payloads and the small
    inline JSON fallback rather than branching on attachment representation
    themselves.
    """

    import batchalign_core

    try:
        raw = batchalign_core.load_worker_json_attachment(attachments, artifact_id)
    except (ValueError, TypeError, BatchalignError) as error:
        raise ArtifactInputErrorV2(str(error)) from error
    return json.loads(raw)


def load_prepared_text_json_v2(attachment: PreparedTextRefV2) -> object:
    """Read one prepared UTF-8 JSON artifact and return the decoded object."""

    import batchalign_core

    try:
        raw = batchalign_core.load_worker_prepared_text_json(attachment)
    except (ValueError, TypeError, BatchalignError) as error:
        raise ArtifactInputErrorV2(str(error)) from error
    return json.loads(raw)


def load_prepared_audio_f32le_v2(attachment: PreparedAudioRefV2) -> np.ndarray:
    """Read one prepared float32 PCM artifact and return a detached numpy array.

    The array shape is:

    - ``(frame_count,)`` for mono audio
    - ``(frame_count, channels)`` for multi-channel audio
    """

    import batchalign_core

    try:
        raw = batchalign_core.load_worker_prepared_audio_f32le_bytes(attachment)
    except (ValueError, TypeError, BatchalignError) as error:
        raise ArtifactInputErrorV2(str(error)) from error
    samples = np.frombuffer(raw, dtype="<f4").copy()
    if attachment.channels == 1:
        return samples
    return samples.reshape((attachment.frame_count, attachment.channels))
