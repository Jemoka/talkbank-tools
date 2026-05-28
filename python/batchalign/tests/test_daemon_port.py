"""Verify the daemon's port-announce contract.

The desktop GUI's sidecar spawner ('apps/batchalign/batchalign-gui/
src-tauri/src/daemon.rs') reads the actual bound port from a single
``DAEMON_PORT=<int>`` line on the daemon's stdout — critical when the
GUI launches with ``--port 0`` so uvicorn picks any free port. This
test pins that contract.
"""

from __future__ import annotations

import re
import socket
import subprocess
import sys
import time
import urllib.error
import urllib.request

import pytest

pytest.importorskip("fastapi")
pytest.importorskip("uvicorn")
# The daemon subprocess imports batchalign.api which pulls sse_starlette;
# match test_api_smoke.py's gate so we skip cleanly when the [api] extra
# isn't on the runtime path.
pytest.importorskip("sse_starlette")


_PORT_LINE = re.compile(rb"^DAEMON_PORT=(\d+)\s*$")


def _free_loopback_port() -> int:
    """Pick a free loopback port for the fixed-port test."""
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def _wait_for_port_line(proc: subprocess.Popen, timeout: float = 15.0) -> int:
    """Read the daemon's stdout line-by-line until DAEMON_PORT=<n> appears."""
    assert proc.stdout is not None
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if proc.poll() is not None:
            raise RuntimeError(
                f"daemon exited (code={proc.returncode}) before announcing port; "
                f"stderr: {proc.stderr.read().decode(errors='replace') if proc.stderr else ''}"
            )
        line = proc.stdout.readline()
        if not line:
            time.sleep(0.05)
            continue
        match = _PORT_LINE.match(line)
        if match:
            return int(match.group(1))
    raise TimeoutError("daemon did not announce DAEMON_PORT within timeout")


@pytest.mark.parametrize("requested_port", [0, "fixed"])
def test_daemon_announces_port(requested_port: int | str) -> None:
    port_arg = _free_loopback_port() if requested_port == "fixed" else 0
    proc = subprocess.Popen(
        [
            sys.executable,
            "-m",
            "batchalign",
            "daemon",
            "--host",
            "127.0.0.1",
            "--port",
            str(port_arg),
            "--log-level",
            "warning",
            "--no-access-log",
        ],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        bufsize=0,
    )
    try:
        bound_port = _wait_for_port_line(proc)
        if requested_port == "fixed":
            assert bound_port == port_arg, (
                f"daemon announced port {bound_port}; expected fixed {port_arg}"
            )
        else:
            assert bound_port > 0
        # Sanity-check the daemon is actually serving on that port.
        for _ in range(50):  # ~5s in 100ms steps
            try:
                with urllib.request.urlopen(
                    f"http://127.0.0.1:{bound_port}/health", timeout=0.5
                ) as resp:
                    assert resp.status == 200
                    break
            except (urllib.error.URLError, ConnectionError):
                time.sleep(0.1)
        else:
            raise AssertionError(f"daemon never served /health on port {bound_port}")
    finally:
        proc.terminate()
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait(timeout=2)
