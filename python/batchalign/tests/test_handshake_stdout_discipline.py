"""Regression: progress events use stdout consistently across the
pre-ready and post-ready windows.

The Rust supervisor's ``read_ready_line``
(``crates/batchalign/src/worker/handle/lifecycle.rs``) accepts
``{"op": "progress_v2", ...}`` lines as bootstrap-time preamble before
the ``{"ready": true, ...}`` envelope, emitting each as
``tracing::info!``. Stderr is buffered until process exit, so
bootstrap-time visibility requires the stdout path. Routing pre-ready
progress events to stderr would break that visibility contract.

(Earlier supervisor versions strictly read one JSON line as the ready
signal and rejected anything else — this test pinned a stderr-routing
contract for that older protocol. The 2026-05-06 supervisor change
relaxed the contract; this test was rewritten to track the new
behavior.)

``_print_ready`` and ``_print_ready_tcp`` flip
``_protocol._handshake_complete`` the moment the ready line is on the
wire so post-ready emissions also use stdout (now via ``_write_json``).
"""

from __future__ import annotations

import io
import json
import sys
from unittest import mock

from batchalign.worker import _protocol


def _reset_handshake_state():
    """Restore the module-level handshake flag for test isolation."""
    _protocol._handshake_complete = False


def test_pre_ready_progress_event_goes_to_stdout_as_preamble():
    """Before ready, ``write_progress_event`` emits a JSON line on stdout.

    The supervisor's ``read_ready_line`` accepts ``progress_v2`` lines
    as bootstrap-time preamble (one or more lines before the ready
    envelope) and forwards each as ``tracing::info!``. Stdout is the
    visibility channel during bootstrap; stderr is buffered until exit.
    """
    _reset_handshake_state()
    fake_stdout = io.StringIO()
    fake_stderr = io.StringIO()
    with (
        mock.patch.object(sys, "stdout", fake_stdout),
        mock.patch.object(sys, "stderr", fake_stderr),
    ):
        _protocol.write_progress_event(
            request_id="",
            completed=0,
            total=0,
            stage="downloading_stanza_catalog",
        )

    line = fake_stdout.getvalue().strip()
    envelope = json.loads(line)
    assert envelope["op"] == "progress_v2", (
        "Pre-ready progress events should be emitted as a single "
        f"progress_v2 JSON line on stdout. Got: {line!r}"
    )
    assert envelope["event"]["stage"] == "downloading_stanza_catalog"
    assert fake_stderr.getvalue() == "", (
        "Stderr is buffered until process exit; pre-ready events must "
        "use the stdout preamble path. Got stderr: "
        f"{fake_stderr.getvalue()!r}"
    )


def test_print_ready_flips_the_flag_and_writes_ready_envelope():
    """``_print_ready()`` writes the ready envelope to stdout and flips the flag.

    After this point, progress events go to stdout normally.
    """
    _reset_handshake_state()
    fake_stdout = io.StringIO()
    fake_stderr = io.StringIO()
    with (
        mock.patch.object(sys, "stdout", fake_stdout),
        mock.patch.object(sys, "stderr", fake_stderr),
    ):
        _protocol._print_ready()

    line = fake_stdout.getvalue().strip()
    envelope = json.loads(line)
    assert envelope["ready"] is True
    assert envelope["transport"] == "stdio"
    assert "pid" in envelope
    assert _protocol._handshake_complete is True


def test_post_ready_progress_event_goes_to_stdout():
    """After ``_print_ready()``, progress events use the normal stdout path.

    This is the contract that lets the runner's ``spawn_progress_forwarder``
    multiplex progress events into the per-job status sink.
    """
    _reset_handshake_state()
    # Manually flip the flag (skip the ready-write so the test isolates
    # post-ready behavior).
    _protocol._handshake_complete = True
    try:
        fake_stdout = io.StringIO()
        fake_stderr = io.StringIO()
        with (
            mock.patch.object(sys, "stdout", fake_stdout),
            mock.patch.object(sys, "stderr", fake_stderr),
        ):
            _protocol.write_progress_event(
                request_id="req-7",
                completed=0,
                total=0,
                stage="downloading_hf_openai_whisper-large-v3",
            )

        line = fake_stdout.getvalue().strip()
        envelope = json.loads(line)
        assert envelope["op"] == "progress_v2"
        assert envelope["event"]["request_id"] == "req-7"
        assert envelope["event"]["stage"] == "downloading_hf_openai_whisper-large-v3"
        # Post-ready should NOT also log to stderr — that would double-
        # report the same event.
        assert fake_stderr.getvalue() == ""
    finally:
        _reset_handshake_state()


def test_print_ready_tcp_flips_the_flag_via_stderr_route():
    """``_print_ready_tcp()`` flips the flag even though ready goes to stderr.

    TCP-mode workers signal readiness on stderr (the CLI launcher reads
    it there); stdout is unused for the handshake. We still flip the
    flag so any code path that calls ``write_progress_event`` post-bind
    behaves consistently across transports.
    """
    _reset_handshake_state()
    fake_stdout = io.StringIO()
    fake_stderr = io.StringIO()
    with (
        mock.patch.object(sys, "stdout", fake_stdout),
        mock.patch.object(sys, "stderr", fake_stderr),
    ):
        _protocol._print_ready_tcp("127.0.0.1", 9100)

    # Ready line went to stderr.
    line = fake_stderr.getvalue().strip()
    envelope = json.loads(line)
    assert envelope["ready"] is True
    assert envelope["transport"] == "tcp"
    assert envelope["port"] == 9100
    # Stdout untouched.
    assert fake_stdout.getvalue() == ""
    # Flag flipped.
    assert _protocol._handshake_complete is True
    _reset_handshake_state()
