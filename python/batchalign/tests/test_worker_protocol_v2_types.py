# affects: python/batchalign/worker/_types_v2.py
# affects: crates/batchalign/src/ipc_schema.rs
"""Cross-language fixture checks for the live worker protocol V2 namespace.

These tests load the canonical fixture files shared with Rust, validate them
through the live Pydantic models in ``batchalign.worker._types_v2``,
and assert that serialization stays byte-for-byte compatible at the JSON value
level. The ``worker_v2`` name remains deliberate while the frozen V1 worker
surface still exists; this suite is the cross-language drift guardrail for that
shared namespace.
"""

from __future__ import annotations

import json
from pathlib import Path

import pytest
from pydantic import BaseModel, ValidationError

from batchalign.worker._types_v2 import (
    AsrElementV2,
    AvqiResultPayloadV2,
    CapabilitiesRequestV2,
    CapabilitiesResponseV2,
    ExecuteResponseV2,
    ExecuteSuccessV2,
    ExecuteRequestV2,
    IndexedWordTimingV2,
    HelloRequestV2,
    HelloResponseV2,
    OpenSmileResultPayloadV2,
    ProgressEventV2,
    ShutdownRequestV2,
    SpeakerSegmentV2,
    WhisperChunkSpanV2,
    WhisperTokenTimingV2,
)

_SCHEMA_MODELS: dict[str, type[BaseModel]] = {
    "hello_request": HelloRequestV2,
    "hello_response": HelloResponseV2,
    "capabilities_request": CapabilitiesRequestV2,
    "capabilities_response": CapabilitiesResponseV2,
    "execute_request": ExecuteRequestV2,
    "execute_response": ExecuteResponseV2,
    "progress_event": ProgressEventV2,
    "shutdown_request": ShutdownRequestV2,
}


def _prune_absent_none_fields(value: object, raw: object) -> object:
    """Drop serialized default fields only when the fixture omitted them.

    Mirrors the Rust ``#[serde(default, skip_serializing_if = ...)]``
    pattern: a field with its default value (``None`` or ``False``) is
    pruned from the model's serialized form when the fixture also
    omits it, so round-tripping a fixture that predates the field
    stays exact-equal.
    """

    if isinstance(value, dict) and isinstance(raw, dict):
        pruned: dict[str, object] = {}
        for key, item in value.items():
            if key not in raw and (item is None or item is False):
                continue
            pruned[key] = _prune_absent_none_fields(item, raw.get(key))
        return pruned
    if isinstance(value, list) and isinstance(raw, list):
        return [
            _prune_absent_none_fields(item, raw[idx] if idx < len(raw) else None)
            for idx, item in enumerate(value)
        ]
    return value


def _fixture_root() -> Path:
    """Return the shared repo-level fixture directory for worker protocol V2."""

    return Path(__file__).resolve().parents[2] / "tests" / "fixtures" / "worker_protocol_v2"


def _load_manifest() -> list[dict[str, str]]:
    """Load the shared fixture manifest used by both test suites."""

    with (_fixture_root() / "manifest.json").open() as handle:
        raw = json.load(handle)
    fixtures = raw["fixtures"]
    assert isinstance(fixtures, list)
    return fixtures


@pytest.mark.parametrize(
    "entry",
    _load_manifest(),
    ids=lambda entry: entry["file"],
)
def test_worker_protocol_v2_fixtures_roundtrip_in_python(entry: dict[str, str]) -> None:
    """Each shared fixture should roundtrip through the live Python V2 schema."""

    schema = entry["schema"]
    model_type = _SCHEMA_MODELS[schema]
    with (_fixture_root() / entry["file"]).open() as handle:
        raw = json.load(handle)

    model = model_type.model_validate(raw)
    assert _prune_absent_none_fields(model.model_dump(mode="json"), raw) == raw


@pytest.mark.parametrize(
    ("model_type", "payload"),
    [
        (
            WhisperChunkSpanV2,
            {"text": "hello", "start_s": 0.5, "end_s": 0.25},
        ),
        (
            WhisperTokenTimingV2,
            {"text": "hello", "time_s": float("nan")},
        ),
        (
            AsrElementV2,
            {"value": "nei5", "start_s": 0.9, "end_s": 0.4, "kind": "text"},
        ),
        (
            IndexedWordTimingV2,
            {"start_ms": 40, "end_ms": 10},
        ),
        (
            SpeakerSegmentV2,
            {"start_ms": 100, "end_ms": 20, "speaker": "SPEAKER_1"},
        ),
    ],
)
def test_worker_protocol_v2_rejects_invalid_ranges(model_type: type[BaseModel], payload: dict[str, object]) -> None:
    """Timing-bearing V2 DTOs should reject reversed spans at the schema boundary."""

    with pytest.raises(ValidationError):
        model_type.model_validate(payload)


def test_worker_protocol_v2_rejects_non_finite_opensmile_metrics() -> None:
    """openSMILE result payloads should reject non-finite numeric rows."""

    with pytest.raises(ValidationError):
        OpenSmileResultPayloadV2.model_validate(
            {
                "feature_set": "eGeMAPSv02",
                "feature_level": "functionals",
                "num_features": 1,
                "duration_segments": 1,
                "audio_file": "/tmp/audio.wav",
                "rows": [{"f0_mean": float("inf")}],
                "success": True,
            }
        )


def test_worker_protocol_v2_rejects_non_finite_avqi_metrics() -> None:
    """AVQI result payloads should reject non-finite metrics."""

    with pytest.raises(ValidationError):
        AvqiResultPayloadV2.model_validate(
            {
                "avqi": float("nan"),
                "cpps": 5.0,
                "hnr": 10.0,
                "shimmer_local": 0.2,
                "shimmer_local_db": 0.3,
                "slope": 0.4,
                "tilt": 0.5,
                "cs_file": "/tmp/sample.cs.wav",
                "sv_file": "/tmp/sample.sv.wav",
                "success": True,
            }
        )


def test_worker_protocol_v2_rejects_non_finite_elapsed_time() -> None:
    """Execute responses should reject non-finite elapsed durations."""

    with pytest.raises(ValidationError):
        ExecuteResponseV2.model_validate(
            {
                "request_id": "req-invalid-elapsed",
                "outcome": ExecuteSuccessV2().model_dump(mode="json"),
                "result": None,
                "elapsed_s": float("nan"),
            }
        )
