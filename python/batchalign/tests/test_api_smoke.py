"""Smoke test for `batchalign.api` — exercise the HTTP surface end-to-end
without requiring heavy ML deps or the compiled `_core` extension.

The "happy path" we can cover here is: discovery → upload → job
submission with validation errors → job submission with a mocked
recipe → SSE drain → result fetch. Real pipeline execution lives in
integration tests that have `maturin develop` + a backend's ML stack
installed.
"""

from __future__ import annotations

import io
import json

import pytest

pytest.importorskip("fastapi")
pytest.importorskip("sse_starlette")

from fastapi.testclient import TestClient

from batchalign.api import JOBS, JobState, app


@pytest.fixture
def client():
    with TestClient(app) as c:
        yield c


def test_health(client):
    r = client.get("/health")
    assert r.status_code == 200
    body = r.json()
    assert body["ok"] is True
    assert "transcribe" in body["recipes"]


def test_capabilities_lists_everything(client):
    r = client.get("/capabilities")
    assert r.status_code == 200
    body = r.json()
    assert "transcribe" in body["recipes"]
    assert "WhisperBackend" in body["backends"]
    assert "ASR" in body["backends_by_task"]
    assert "WhisperBackend" in body["backends_by_task"]["ASR"]


def test_upload_returns_id(client, tmp_path):
    payload = b"fake media bytes"
    r = client.post(
        "/uploads",
        files={"file": ("clip.wav", io.BytesIO(payload), "audio/wav")},
    )
    assert r.status_code == 200
    upload_id = r.json()["upload_id"]
    assert upload_id.endswith(".wav")

    # Delete works.
    r2 = client.delete(f"/uploads/{upload_id}")
    assert r2.status_code == 200


def test_unknown_backend_kind_400(client, tmp_path):
    # Upload first so we have a valid input_id.
    up = client.post(
        "/uploads",
        files={"file": ("clip.wav", io.BytesIO(b"x"), "audio/wav")},
    ).json()
    body = {
        "asr_backend": {"kind": "NotARealBackend", "kwargs": {}},
        "inputs": [{"kind": "media", "upload_id": up["upload_id"]}],
    }
    r = client.post("/recipes/transcribe", json=body)
    assert r.status_code == 400
    assert "NotARealBackend" in r.json()["detail"]


def test_missing_required_backend_field_422(client):
    # `transcribe` requires `asr_backend`.
    r = client.post(
        "/recipes/transcribe",
        json={"inputs": [{"kind": "media", "upload_id": "anything"}]},
    )
    assert r.status_code == 422


def test_job_lifecycle_with_stub_recipe(client, monkeypatch, tmp_path):
    """End-to-end without `_core`: monkey-patch `RECIPES["transcribe"]`
    with a stub that returns a Pipeline-like object whose `.run()`
    fires one progress event and returns a fake outcome."""
    from batchalign import api as api_mod

    upload_resp = client.post(
        "/uploads",
        files={"file": ("clip.wav", io.BytesIO(b"x"), "audio/wav")},
    ).json()

    class FakeEvent:
        source_id = "clip"
        kind = "StageStarted"
        task = None
        completed = 1
        total = 2
        label = "asr"

    class FakeOutcome:
        source_id = "clip"
        kind = "ok"
        completed = 2
        total = 2
        label = "done"

    class FakePipeline:
        def run(self, inputs, callbacks):
            for inp in inputs:
                sid = str(getattr(inp, "source_id", "") or "")
                callbacks[sid](FakeEvent())
            return [FakeOutcome()]

    def fake_recipe(*, asr_backend, speaker_backend=None, **opts):
        return FakePipeline()

    monkeypatch.setitem(api_mod.RECIPES, "transcribe", fake_recipe)

    # Build minimal request. `asr_backend` is required by transcribe's
    # request model; use the simplest real backend (Stanza needs nothing
    # heavy to *construct* the spec — kwargs aren't validated until
    # construction time, and the fake_recipe ignores it anyway).
    body = {
        "asr_backend": {"kind": "WhisperBackend", "kwargs": {"model": "noop"}},
        "inputs": [
            {"kind": "media", "upload_id": upload_resp["upload_id"], "source_id": "clip"}
        ],
    }
    # Because the fake_recipe never touches asr_backend, we don't want
    # `build_backend` to actually construct WhisperBackend (which would
    # import transformers). Patch it to a sentinel.
    monkeypatch.setattr(api_mod, "build_backend", lambda spec: object())

    r = client.post("/recipes/transcribe", json=body)
    assert r.status_code == 200, r.text
    job_id = r.json()["job_id"]

    # Drain SSE until "done". sse_starlette emits SSE-style
    # `event: progress\ndata: ...\n\n` frames; we don't care about the
    # exact line shape, just that progress is observed before done.
    with client.stream("GET", f"/jobs/{job_id}/events") as stream:
        text = ""
        for line in stream.iter_lines():
            text += line + "\n"
            if "done" in line:
                break
        assert "progress" in text, f"no progress event in: {text!r}"

    # Status reflects completion.
    state = client.get(f"/jobs/{job_id}").json()["state"]
    assert state == JobState.COMPLETED.value

    # Result is fetchable.
    result = client.get(f"/jobs/{job_id}/result").json()
    assert result["job_id"] == job_id
    assert len(result["outcomes"]) == 1

    # Cleanup.
    JOBS.pop(job_id, None)
