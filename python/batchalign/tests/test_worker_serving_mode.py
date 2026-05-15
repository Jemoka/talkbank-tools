"""Tests for GPU worker serving-mode selection (concurrent vs sequential).

GPU profile workers use ThreadPoolExecutor for concurrent inference ONLY when
CUDA is available — PyTorch releases the GIL during CUDA kernels, enabling
real parallelism. On CPU (Apple Silicon with MPS excluded, or any non-CUDA
machine), the worker falls back to sequential serving to avoid OpenMP thread
oversubscription.

These tests verify the decision logic in ``main()`` without loading any ML
models — they monkeypatch the bootstrap state and serving functions to check
which mode was selected.
"""

from __future__ import annotations

import logging
from dataclasses import dataclass
from types import SimpleNamespace

import pytest

from batchalign.worker._main import _gpu_has_cuda_device


# ---------------------------------------------------------------------------
# _gpu_has_cuda_device unit tests
# ---------------------------------------------------------------------------


class TestGpuHasCudaDevice:
    """Direct tests for the CUDA detection helper."""

    def test_returns_true_when_cuda_available_and_not_force_cpu(
        self, monkeypatch,
    ) -> None:
        monkeypatch.setattr("torch.cuda.is_available", lambda: True)
        assert _gpu_has_cuda_device(force_cpu=False) is True

    def test_returns_false_when_no_cuda(self, monkeypatch) -> None:
        monkeypatch.setattr("torch.cuda.is_available", lambda: False)
        assert _gpu_has_cuda_device(force_cpu=False) is False

    def test_returns_false_when_cuda_available_but_force_cpu(
        self, monkeypatch,
    ) -> None:
        monkeypatch.setattr("torch.cuda.is_available", lambda: True)
        assert _gpu_has_cuda_device(force_cpu=True) is False

    def test_returns_false_when_no_cuda_and_force_cpu(
        self, monkeypatch,
    ) -> None:
        monkeypatch.setattr("torch.cuda.is_available", lambda: False)
        assert _gpu_has_cuda_device(force_cpu=True) is False


# ---------------------------------------------------------------------------
# Serving-mode integration tests
# ---------------------------------------------------------------------------
# These test the full decision path in main() by monkeypatching model loading,
# CUDA detection, and the serving functions themselves.


@dataclass
class _ServingCapture:
    """Tracks which serving function was called."""

    called: str = ""
    max_threads: int | None = None


def _make_main_args(
    *,
    profile: str = "gpu",
    transport: str = "stdio",
    force_cpu: bool = False,
    gpu_thread_pool_size: int = 4,
) -> SimpleNamespace:
    """Build a fake argparse namespace matching ``build_arg_parser()``."""
    return SimpleNamespace(
        task="",
        lang="eng",
        num_speakers=1,
        engine_overrides="",
        test_echo=False,
        test_delay_ms=0,
        verbose=0,
        profile=profile,
        force_cpu=force_cpu,
        gpu_thread_pool_size=gpu_thread_pool_size,
        transport=transport,
        host="127.0.0.1",
        port=0,
        lazy=False,
    )


def _patch_main_for_serving_test(
    monkeypatch,
    *,
    cuda_available: bool,
    capture: _ServingCapture,
    args: SimpleNamespace,
) -> None:
    """Monkeypatch everything ``main()`` calls so we can observe the
    serving-mode decision without loading any ML models."""
    from batchalign.device import DevicePolicy
    from batchalign.worker._types import WorkerBootstrapRuntime, WorkerProfile, _state

    # Fake bootstrap — pretend model loading succeeded
    profile = WorkerProfile(args.profile) if args.profile else None
    bootstrap = WorkerBootstrapRuntime(
        task=None,
        lang=args.lang,
        num_speakers=args.num_speakers,
        profile=profile,
        device_policy=DevicePolicy(force_cpu=args.force_cpu),
    )
    _state.bootstrap = bootstrap

    # Patch argument parsing to return our fake args
    monkeypatch.setattr(
        "batchalign.worker._main.build_arg_parser",
        lambda: SimpleNamespace(parse_args=lambda: args),
    )

    # Patch model loading to no-op
    monkeypatch.setattr(
        "batchalign.worker._main.load_worker_profile",
        lambda _bootstrap: None,
    )

    # Patch CUDA detection
    monkeypatch.setattr("torch.cuda.is_available", lambda: cuda_available)

    # Patch serving functions to record which was called
    def _fake_serve_stdio():
        capture.called = "sequential_stdio"

    def _fake_serve_stdio_concurrent(max_threads: int = 4):
        capture.called = "concurrent_stdio"
        capture.max_threads = max_threads

    def _fake_serve_tcp(host: str, port: int):
        capture.called = "sequential_tcp"

    def _fake_serve_tcp_concurrent(host: str, port: int, max_threads: int = 4):
        capture.called = "concurrent_tcp"
        capture.max_threads = max_threads

    monkeypatch.setattr("batchalign.worker._main._serve_stdio", _fake_serve_stdio)
    monkeypatch.setattr(
        "batchalign.worker._main._serve_stdio_concurrent",
        _fake_serve_stdio_concurrent,
    )
    monkeypatch.setattr("batchalign.worker._main._serve_tcp", _fake_serve_tcp)
    monkeypatch.setattr(
        "batchalign.worker._main._serve_tcp_concurrent",
        _fake_serve_tcp_concurrent,
    )
    monkeypatch.setattr("batchalign.worker._main._print_ready", lambda: None)
    monkeypatch.setattr(
        "batchalign.worker._main._auto_assign_port",
        lambda _host: 9100,
    )
    # Suppress logging output — use a real logger at CRITICAL level instead of
    # unittest.mock (which is banned in this codebase).
    silent_logger = logging.getLogger("test.worker.serving_mode.null")
    silent_logger.setLevel(logging.CRITICAL)
    monkeypatch.setattr("batchalign.worker._main.L", silent_logger)


class TestServingModeSelection:
    """Verify that main() selects the correct serving mode based on
    GPU profile + CUDA availability + force_cpu flag."""

    def test_gpu_profile_on_cpu_uses_sequential_serving(
        self, monkeypatch,
    ) -> None:
        """GPU profile without CUDA should use sequential serving to avoid
        OpenMP thread oversubscription on CPU."""
        capture = _ServingCapture()
        args = _make_main_args(profile="gpu")
        _patch_main_for_serving_test(
            monkeypatch, cuda_available=False, capture=capture, args=args,
        )

        from batchalign.worker._main import main
        main()

        assert capture.called == "sequential_stdio"

    def test_gpu_profile_on_cuda_uses_concurrent_serving(
        self, monkeypatch,
    ) -> None:
        """GPU profile with CUDA should use concurrent serving — PyTorch
        releases the GIL during CUDA kernels."""
        capture = _ServingCapture()
        args = _make_main_args(profile="gpu", gpu_thread_pool_size=8)
        _patch_main_for_serving_test(
            monkeypatch, cuda_available=True, capture=capture, args=args,
        )

        from batchalign.worker._main import main
        main()

        assert capture.called == "concurrent_stdio"
        assert capture.max_threads == 8

    def test_gpu_profile_force_cpu_uses_sequential_serving(
        self, monkeypatch,
    ) -> None:
        """GPU profile with CUDA available but --force-cpu should use
        sequential serving — models are on CPU regardless of CUDA."""
        capture = _ServingCapture()
        args = _make_main_args(profile="gpu", force_cpu=True)
        _patch_main_for_serving_test(
            monkeypatch, cuda_available=True, capture=capture, args=args,
        )

        from batchalign.worker._main import main
        main()

        assert capture.called == "sequential_stdio"

    def test_stanza_profile_always_uses_sequential_serving(
        self, monkeypatch,
    ) -> None:
        """Stanza profile should always use sequential serving regardless
        of CUDA availability — Stanza is CPU-bound and GIL-limited."""
        capture = _ServingCapture()
        args = _make_main_args(profile="stanza")
        _patch_main_for_serving_test(
            monkeypatch, cuda_available=True, capture=capture, args=args,
        )

        from batchalign.worker._main import main
        main()

        assert capture.called == "sequential_stdio"

    def test_gpu_profile_tcp_on_cpu_uses_sequential_tcp(
        self, monkeypatch,
    ) -> None:
        """GPU profile over TCP without CUDA should use sequential TCP
        serving, not concurrent."""
        capture = _ServingCapture()
        args = _make_main_args(profile="gpu", transport="tcp")
        _patch_main_for_serving_test(
            monkeypatch, cuda_available=False, capture=capture, args=args,
        )

        from batchalign.worker._main import main
        main()

        assert capture.called == "sequential_tcp"

    def test_gpu_profile_tcp_on_cuda_uses_concurrent_tcp(
        self, monkeypatch,
    ) -> None:
        """GPU profile over TCP with CUDA should use concurrent TCP serving."""
        capture = _ServingCapture()
        args = _make_main_args(profile="gpu", transport="tcp")
        _patch_main_for_serving_test(
            monkeypatch, cuda_available=True, capture=capture, args=args,
        )

        from batchalign.worker._main import main
        main()

        assert capture.called == "concurrent_tcp"
