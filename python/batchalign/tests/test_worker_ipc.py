"""Tests for the infer-era worker IPC contract between Rust and Python.

Cross-language contract: this is the Python half. The Rust half lives in
``crates/batchalign/tests/worker_protocol_v2_compat.rs``. Both sides
must independently verify that the wire format roundtrips correctly — a
change to an IPC type must update both Rust and Python models.
"""

from __future__ import annotations

import json

import pytest
from pydantic import ValidationError

from batchalign.worker import (
    BatchInferRequest,
    BatchInferResponse,
    CapabilitiesResponse,
    HealthResponse,
    InferTask,
    InferRequest,
    InferResponse,
)


def test_health_response() -> None:
    """HealthResponse serializes with expected fields."""
    resp = HealthResponse(
        status="ok",
        command="infer:morphosyntax",
        lang="eng",
        pid=12345,
        uptime_s=120.5,
    )
    data = json.loads(resp.model_dump_json())
    assert data["status"] == "ok"
    assert data["command"] == "infer:morphosyntax"
    assert data["pid"] == 12345


def test_capabilities_response() -> None:
    """CapabilitiesResponse serializes with expected fields."""
    resp = CapabilitiesResponse(
        commands=["align", "morphotag", "opensmile"],
        free_threaded=True,
        infer_tasks=[],
        engine_versions={},
    )
    data = json.loads(resp.model_dump_json())
    assert data["commands"] == ["align", "morphotag", "opensmile"]
    assert data["free_threaded"] is True


def test_capabilities_response_with_infer_fields() -> None:
    """CapabilitiesResponse includes infer_tasks and engine_versions."""
    resp = CapabilitiesResponse(
        commands=["morphotag"],
        free_threaded=False,
        infer_tasks=[InferTask.MORPHOSYNTAX, InferTask.UTSEG],
        engine_versions={
            InferTask.MORPHOSYNTAX: "stanza-1.9.2",
            InferTask.UTSEG: "stanza-1.9.2",
        },
    )
    data = json.loads(resp.model_dump_json())
    assert data["infer_tasks"] == ["morphosyntax", "utseg"]
    assert data["engine_versions"]["morphosyntax"] == "stanza-1.9.2"


def test_capabilities_response_missing_infer_fields_is_rejected() -> None:
    """CapabilitiesResponse requires infer_tasks and engine_versions."""
    with pytest.raises(ValidationError):
        CapabilitiesResponse(commands=["morphotag"], free_threaded=False)


def test_infer_request_serialization() -> None:
    """InferRequest serializes with task, lang, and payload."""
    req = InferRequest(
        task=InferTask.MORPHOSYNTAX,
        lang="eng",
        payload={"words": ["the", "dog", "runs"], "terminator": "."},
    )
    data = json.loads(req.model_dump_json())
    assert data["task"] == "morphosyntax"
    assert data["lang"] == "eng"
    assert data["payload"]["words"] == ["the", "dog", "runs"]


def test_infer_response_success() -> None:
    """InferResponse on success has result, no error."""
    resp = InferResponse(
        result={"mor": "det|the n|dog v|run-3S", "gra": "1|2|DET 2|3|SUBJ 3|0|ROOT"},
        elapsed_s=0.5,
    )
    data = json.loads(resp.model_dump_json())
    assert data["result"]["mor"].startswith("det|the")
    assert data["error"] is None
    assert data["elapsed_s"] == 0.5


def test_infer_response_error() -> None:
    """InferResponse on error has error message, no result."""
    resp = InferResponse(
        error="infer task 'morphosyntax' not yet implemented",
        elapsed_s=0.0,
    )
    data = json.loads(resp.model_dump_json())
    assert data["result"] is None
    assert "not yet implemented" in data["error"]


def test_batch_infer_request_serialization() -> None:
    """BatchInferRequest serializes items as a list."""
    req = BatchInferRequest(
        task=InferTask.MORPHOSYNTAX,
        lang="eng",
        items=[
            {"words": ["hello"], "terminator": "."},
            {"words": ["world"], "terminator": "."},
        ],
    )
    data = json.loads(req.model_dump_json())
    assert data["task"] == "morphosyntax"
    assert len(data["items"]) == 2
    assert data["items"][0]["words"] == ["hello"]


def test_batch_infer_response_serialization() -> None:
    """BatchInferResponse contains a list of InferResponse."""
    resp = BatchInferResponse(
        results=[
            InferResponse(result={"mor": "n|hello"}, elapsed_s=0.1),
            InferResponse(error="failed", elapsed_s=0.0),
        ],
    )
    data = json.loads(resp.model_dump_json())
    assert len(data["results"]) == 2
    assert data["results"][0]["result"]["mor"] == "n|hello"
    assert data["results"][1]["error"] == "failed"


def test_rust_can_parse_python_infer_request() -> None:
    """Verify the JSON shape Python produces matches what Rust expects."""
    rust_json = json.dumps({
        "task": "morphosyntax",
        "lang": "eng",
        "payload": {"words": ["hello", "world"], "terminator": "."},
    })
    req = InferRequest.model_validate_json(rust_json)
    assert req.task == InferTask.MORPHOSYNTAX
    assert req.payload["words"] == ["hello", "world"]  # type: ignore[index]


def test_python_can_parse_rust_infer_response() -> None:
    """Verify Python can parse the JSON that Rust produces for InferResponse."""
    rust_json = json.dumps({
        "result": {"mor": "n|hello n|world", "gra": "1|2|SUBJ 2|0|ROOT"},
        "error": None,
        "elapsed_s": 0.123,
    })
    resp = InferResponse.model_validate_json(rust_json)
    assert resp.result is not None
    assert resp.error is None
    assert resp.elapsed_s == 0.123

