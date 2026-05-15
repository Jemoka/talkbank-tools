"""Tests that ``_serve_stdio`` catches dispatch exceptions and emits a
structured error response with the right ``kind`` discriminator.

Pre-fix behavior: an uncaught exception in the dispatch path killed the
Python worker (exit code 1). The Rust orchestrator saw ``ProcessExited`` →
``WorkerCrash`` → retryable, retried up to 3× with a full traceback per
attempt, and a single-host instance of this loop produced multi-GB
``server.log`` spam from a deterministic Stanza-catalog miss.

Post-fix behavior:

- Bootstrap-class exceptions (``StanzaCatalogDownloadError``,
  ``UnsupportedLanguageError``) → emit
  ``{"op": "error", "error": "...", "kind": "bootstrap"}``. The Rust
  side classifies as terminal and surfaces the verbatim message to the
  user.
- Other exceptions → emit ``{"op": "error", "error": "...", "kind":
  "runtime"}``. The Rust side keeps existing retry semantics.

The worker process never dies from a per-request exception — except when
``kind == "bootstrap"``, in which case the worker DOES exit (cleanly,
after emitting the error) so the pool tears it down. That exit is safe
because the orchestrator has already classified the error as
non-retryable, so no retry storm follows.
"""

from __future__ import annotations

import io
import json
import sys
from unittest import mock

from batchalign.worker._protocol import (
    _classify_dispatch_exception,
    _serve_stdio,
)


# ---------------------------------------------------------------------------
# Classifier: bootstrap-vs-runtime discriminator on raw exception types.
# ---------------------------------------------------------------------------


def test_classify_stanza_catalog_download_error_is_bootstrap():
    """A typed catalog-download failure must classify as bootstrap-kind."""
    from batchalign.worker._stanza_capabilities import (
        StanzaCatalogDownloadError,
    )

    exc = StanzaCatalogDownloadError("network unreachable")
    assert _classify_dispatch_exception(exc) == "bootstrap"


def test_classify_unsupported_language_error_is_bootstrap():
    """Genuinely-unsupported language is also a bootstrap-class failure.

    Reasoning: it's deterministic (the same language will be unsupported
    on every retry), and the user-facing remediation is the same actionable
    "this language isn't supported by your Stanza install" message.
    """
    from batchalign.worker._stanza_loading import UnsupportedLanguageError

    exc = UnsupportedLanguageError("language 'que' not supported")
    assert _classify_dispatch_exception(exc) == "bootstrap"


def test_classify_generic_runtime_error_is_runtime():
    """A bare exception (not a typed bootstrap error) classifies as runtime."""
    exc = RuntimeError("something went wrong mid-inference")
    assert _classify_dispatch_exception(exc) == "runtime"


def test_classify_value_error_is_runtime():
    """Validation failures (ValueError on a malformed input) are per-request."""
    exc = ValueError("invalid utterance payload")
    assert _classify_dispatch_exception(exc) == "runtime"


# ---------------------------------------------------------------------------
# _serve_stdio integration: dispatch exception must produce a structured wire
# response, not a process exit.
# ---------------------------------------------------------------------------


def _run_serve_stdio_with_one_message(message_json: str, dispatch_side_effect):
    """Drive ``_serve_stdio`` with a single input line and a mocked dispatch.

    Returns the captured stdout as a list of decoded JSON envelopes (one
    per emitted line).
    """
    fake_stdin = io.StringIO(message_json + "\n")
    fake_stdout = io.StringIO()
    fake_stderr = io.StringIO()

    with (
        mock.patch.object(sys, "stdin", fake_stdin),
        mock.patch.object(sys, "stdout", fake_stdout),
        mock.patch.object(sys, "stderr", fake_stderr),
        mock.patch(
            "batchalign.worker._protocol.dispatch_protocol_message",
            side_effect=dispatch_side_effect,
        ),
    ):
        _serve_stdio()

    lines = [
        line for line in fake_stdout.getvalue().splitlines() if line.strip()
    ]
    return [json.loads(line) for line in lines]


def test_serve_stdio_emits_bootstrap_kind_on_typed_bootstrap_error():
    """A bootstrap-class exception must produce a ``kind=bootstrap`` envelope.

    This is the canonical regression for the bootstrap-retry-classification
    fix. Pre-fix, this exception would propagate up and kill the worker;
    post-fix, the worker emits a structured error and the Rust classifier
    (separately tested in Rust) treats it as terminal.
    """
    from batchalign.worker._stanza_capabilities import (
        StanzaCatalogDownloadError,
    )

    def fake_dispatch(_message):
        raise StanzaCatalogDownloadError(
            "Failed to download Stanza resource catalog from "
            "https://raw.githubusercontent.com/stanfordnlp/stanza-resources/main: "
            "connection refused"
        )

    envelopes = _run_serve_stdio_with_one_message(
        '{"op": "ensure_task", "request": {"task": "morphosyntax"}}',
        fake_dispatch,
    )

    assert len(envelopes) == 1, (
        f"Expected exactly one error envelope; got: {envelopes}"
    )
    env = envelopes[0]
    assert env["op"] == "error"
    assert env["kind"] == "bootstrap"
    assert "connection refused" in env["error"]


def test_serve_stdio_emits_runtime_kind_on_generic_error():
    """A non-bootstrap exception must default to ``kind=runtime``.

    Existing retry semantics for transient per-request failures
    (different inputs, transient resource state, external-API hiccups)
    rely on the runtime kind. We don't want to suddenly classify all
    handler exceptions as terminal.
    """
    def fake_dispatch(_message):
        raise RuntimeError("transient inference error")

    envelopes = _run_serve_stdio_with_one_message(
        '{"op": "infer", "request": {}}',
        fake_dispatch,
    )

    assert len(envelopes) == 1
    env = envelopes[0]
    assert env["op"] == "error"
    assert env["kind"] == "runtime"
    assert "transient inference error" in env["error"]


def test_serve_stdio_continues_after_runtime_error():
    """After a runtime-kind error, ``_serve_stdio`` must keep reading input.

    Pre-fix, an uncaught exception killed the loop (and the worker
    process). Post-fix, runtime errors should not abort the loop — only
    bootstrap errors do.
    """
    call_count = {"n": 0}

    def fake_dispatch(_message):
        call_count["n"] += 1
        if call_count["n"] == 1:
            raise RuntimeError("first call fails")
        # Second call returns a valid envelope.
        from batchalign.worker._protocol_ops import ProtocolDispatchResult

        return ProtocolDispatchResult(payload={"op": "health", "ok": True})

    fake_stdin = io.StringIO(
        '{"op": "infer", "request": {}}\n'
        + '{"op": "health"}\n'
    )
    fake_stdout = io.StringIO()
    fake_stderr = io.StringIO()
    with (
        mock.patch.object(sys, "stdin", fake_stdin),
        mock.patch.object(sys, "stdout", fake_stdout),
        mock.patch.object(sys, "stderr", fake_stderr),
        mock.patch(
            "batchalign.worker._protocol.dispatch_protocol_message",
            side_effect=fake_dispatch,
        ),
    ):
        _serve_stdio()

    lines = [
        line for line in fake_stdout.getvalue().splitlines() if line.strip()
    ]
    assert len(lines) == 2, (
        f"Both messages must produce envelopes; got {len(lines)}: {lines}"
    )
    env1 = json.loads(lines[0])
    env2 = json.loads(lines[1])
    assert env1["op"] == "error"
    assert env1["kind"] == "runtime"
    assert env2["op"] == "health"


def test_serve_stdio_exits_after_bootstrap_error():
    """A bootstrap-kind error must terminate the loop after emitting.

    A worker that hit a bootstrap failure is in a partially-initialized
    state — the safest move is to exit cleanly so the pool spawns a fresh
    worker. The orchestrator's terminal classification of bootstrap
    errors prevents retry-storm cascades from this exit.
    """
    call_count = {"n": 0}

    def fake_dispatch(_message):
        from batchalign.worker._stanza_loading import UnsupportedLanguageError

        call_count["n"] += 1
        raise UnsupportedLanguageError("que not supported")

    fake_stdin = io.StringIO(
        '{"op": "ensure_task", "request": {"task": "morphosyntax"}}\n'
        + '{"op": "infer", "request": {}}\n'  # Should never be reached.
    )
    fake_stdout = io.StringIO()
    fake_stderr = io.StringIO()
    with (
        mock.patch.object(sys, "stdin", fake_stdin),
        mock.patch.object(sys, "stdout", fake_stdout),
        mock.patch.object(sys, "stderr", fake_stderr),
        mock.patch(
            "batchalign.worker._protocol.dispatch_protocol_message",
            side_effect=fake_dispatch,
        ),
    ):
        _serve_stdio()

    assert call_count["n"] == 1, (
        f"Loop must exit after the first bootstrap error; got "
        f"{call_count['n']} dispatch calls"
    )
    lines = [
        line for line in fake_stdout.getvalue().splitlines() if line.strip()
    ]
    assert len(lines) == 1
    env = json.loads(lines[0])
    assert env["op"] == "error"
    assert env["kind"] == "bootstrap"
