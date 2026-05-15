# affects: python/batchalign/worker/_main.py
# affects: python/batchalign/worker/_types_v2.py
"""End-to-end tests for the Python worker subprocess.

These tests spawn a real `python -m batchalign.worker --test-echo --task morphosyntax` process
and communicate via stdio JSON-lines — the exact IPC boundary that the
Rust server uses. They verify startup, health, capabilities, infer,
batch_infer, and shutdown without loading any ML models.

Marked @pytest.mark.integration because they spawn subprocesses.
"""

from __future__ import annotations
from typing import Any

import json
import shutil
import subprocess
import sys

import pytest


def _start_test_echo_worker() -> subprocess.Popen[str]:
    """Start a worker in test-echo mode and wait for the ready signal."""
    proc = subprocess.Popen(
        [
            sys.executable,
            "-m",
            "batchalign.worker",
            "--test-echo",
            "--task",
            "morphosyntax",
        ],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=1,  # line-buffered
    )

    assert proc.stdout is not None
    ready_line = proc.stdout.readline()
    assert ready_line, "Worker should emit a ready line on stdout"
    ready = json.loads(ready_line)
    assert ready.get("ready") is True, f"Expected ready=true, got: {ready}"
    assert "pid" in ready, "Ready signal should include pid"
    assert ready.get("transport") == "stdio", "Transport should be stdio"

    return proc


def _send_recv(
    proc: subprocess.Popen[str], request: dict[str, Any],
) -> dict[str, Any]:
    """Send a JSON-lines request and read the response envelope.

    Worker responses are wrapped: {"op": "<op>", "response": {...}}.
    This function returns the inner "response" dict.
    """
    assert proc.stdin is not None
    assert proc.stdout is not None

    line = json.dumps(request) + "\n"
    proc.stdin.write(line)
    proc.stdin.flush()

    response_line = proc.stdout.readline()
    assert response_line, f"Worker should respond to {request.get('op')}"
    envelope: dict[str, Any] = json.loads(response_line)
    assert "response" in envelope, f"Expected 'response' key in envelope: {envelope}"
    inner = envelope["response"]
    assert isinstance(inner, dict)
    return inner


def _shutdown(proc: subprocess.Popen[str]) -> None:
    """Send shutdown and wait for clean exit."""
    assert proc.stdin is not None
    proc.stdin.write(json.dumps({"op": "shutdown"}) + "\n")
    proc.stdin.flush()
    proc.wait(timeout=5)


def _require_uv() -> str:
    """Return the uv executable path or skip when unavailable."""
    uv = shutil.which("uv")
    if uv is None:
        pytest.skip("uv is required for console-script integration tests")
    return uv


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------


@pytest.mark.integration
def test_worker_ready_signal() -> None:
    """Worker emits ready signal with pid and transport on startup."""
    proc = _start_test_echo_worker()
    try:
        # If we got here, the ready signal was valid (asserted in helper)
        assert proc.poll() is None, "Worker should still be running"
    finally:
        _shutdown(proc)


@pytest.mark.integration
def test_worker_health() -> None:
    """Health op returns status=ok."""
    proc = _start_test_echo_worker()
    try:
        resp = _send_recv(proc, {"op": "health"})
        assert resp.get("status") == "ok", f"Expected status=ok, got: {resp}"
    finally:
        _shutdown(proc)


@pytest.mark.integration
def test_worker_capabilities() -> None:
    """Capabilities op returns commands list and empty infer_tasks for test-echo."""
    proc = _start_test_echo_worker()
    try:
        resp = _send_recv(proc, {"op": "capabilities"})
        assert "commands" in resp, f"Expected commands in response: {resp}"
        commands = resp["commands"]
        assert isinstance(commands, list)
        assert len(commands) > 0, "Should advertise at least some commands"
        # Test-echo should include common commands
        assert "morphotag" in commands, "morphotag should be in commands"
        assert "transcribe" in commands, "transcribe should be in commands"
        assert "test-echo" in commands, "test-echo should be in commands"
        # Test-echo workers advertise all infer tasks so the server capability gate passes
        infer_tasks = resp.get("infer_tasks", [])
        assert len(infer_tasks) > 0, "test-echo should advertise all infer tasks"
        assert set(infer_tasks) == {
            "morphosyntax", "utseg", "coref", "translate", "fa", 
            "speaker", "opensmile", "avqi", "asr"
        }, f"Unexpected infer_tasks: {infer_tasks}"
        # Test-echo engine versions should all be "test-echo"
        engine_versions = resp.get("engine_versions", {})
        assert all(v == "test-echo" for v in engine_versions.values()), (
            "test-echo engine_versions should all be 'test-echo'"
        )
    finally:
        _shutdown(proc)


@pytest.mark.integration
def test_worker_batch_infer_echo() -> None:
    """batch_infer op in test-echo mode returns items unchanged."""
    proc = _start_test_echo_worker()
    try:
        items = [
            {"words": ["hello", "world"], "lang": "eng"},
            {"words": ["goodbye"], "lang": "eng"},
        ]
        request = {
            "op": "batch_infer",
            "request": {
                "task": "morphosyntax",
                "lang": "eng",
                "items": items,
            },
        }
        resp = _send_recv(proc, request)
        results = resp.get("results")
        assert isinstance(results, list)
        assert len(results) == 2, f"Expected 2 results, got {len(results)}"
        # Test-echo echoes each item back as the result
        assert results[0].get("result") == items[0]
        assert results[1].get("result") == items[1]
    finally:
        _shutdown(proc)


@pytest.mark.integration
def test_worker_infer_echo() -> None:
    """infer op in test-echo mode returns payload unchanged."""
    proc = _start_test_echo_worker()
    try:
        payload = {"words": ["hello"], "lang": "eng"}
        request = {
            "op": "infer",
            "request": {
                "task": "morphosyntax",
                "lang": "eng",
                "payload": payload,
            },
        }
        resp = _send_recv(proc, request)
        assert resp.get("result") == payload, "Echo should return payload unchanged"
        assert resp.get("error") is None
    finally:
        _shutdown(proc)


@pytest.mark.integration
def test_worker_shutdown_clean_exit() -> None:
    """Shutdown op causes clean process exit."""
    proc = _start_test_echo_worker()
    assert proc.stdin is not None
    proc.stdin.write(json.dumps({"op": "shutdown"}) + "\n")
    proc.stdin.flush()

    exit_code = proc.wait(timeout=5)
    assert exit_code == 0, f"Worker should exit cleanly, got code {exit_code}"


@pytest.mark.integration
def test_worker_multiple_requests() -> None:
    """Worker handles multiple sequential requests correctly."""
    proc = _start_test_echo_worker()
    try:
        # Send health
        resp1 = _send_recv(proc, {"op": "health"})
        assert resp1.get("status") == "ok"

        # Send infer
        payload = {"words": ["hello"], "lang": "eng"}
        resp2 = _send_recv(proc, {
            "op": "infer",
            "request": {
                "task": "morphosyntax",
                "lang": "eng",
                "payload": payload,
            },
        })
        assert resp2.get("result") == payload

        # Send capabilities
        resp3 = _send_recv(proc, {"op": "capabilities"})
        assert "commands" in resp3

        # Send batch_infer
        items = [{"words": ["hello", "again"]}]
        resp4 = _send_recv(proc, {
            "op": "batch_infer",
            "request": {
                "task": "morphosyntax",
                "lang": "eng",
                "items": items,
            },
        })
        assert resp4.get("results")[0]["result"] == items[0]
    finally:
        _shutdown(proc)


@pytest.mark.integration
def test_console_script_help_smoke() -> None:
    """Installed batchalign3 console script should expose the Rust CLI help."""
    uv = _require_uv()
    result = subprocess.run(
        [uv, "run", "--no-sync", "batchalign3", "--help"],
        capture_output=True,
        text=True,
        check=False,
    )

    assert result.returncode == 0, (
        f"expected batchalign3 --help to succeed\n"
        f"stdout:\n{result.stdout}\n"
        f"stderr:\n{result.stderr}"
    )
    assert "Usage: batchalign3 [OPTIONS] <COMMAND>" in result.stdout
    assert "align" in result.stdout
    assert "serve" in result.stdout
    assert result.stderr == ""


@pytest.mark.integration
def test_console_script_invalid_command_propagates_parse_failure() -> None:
    """Installed batchalign3 console script should preserve clap parse failures."""
    uv = _require_uv()
    result = subprocess.run(
        [uv, "run", "--no-sync", "batchalign3", "definitely-not-a-command"],
        capture_output=True,
        text=True,
        check=False,
    )

    assert result.returncode != 0, "invalid command should fail"
    assert result.stdout == ""
    assert "unrecognized subcommand 'definitely-not-a-command'" in result.stderr
    assert "Usage: batchalign3 [OPTIONS] <COMMAND>" in result.stderr
