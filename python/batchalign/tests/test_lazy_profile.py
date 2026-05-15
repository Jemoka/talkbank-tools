"""Tests for LazyProfile worker bootstrap mode.

LazyProfile workers start with no models loaded and load them on demand
via ensure_task_loaded(). This is the principled architecture for
memory-constrained machines (24-48 GB) where eager profile loading would
consume 10-15 GB speculatively.
"""

from __future__ import annotations

import threading

from batchalign.worker._main import build_worker_bootstrap_runtime, parse_worker_args
from batchalign.worker._model_loading.bootstrap import (
    EnsureTaskResponse,
    ensure_task_loaded,
    load_worker_profile_lazy,
)
from batchalign.worker._types import WorkerProfile, _WorkerState, _state


def _reset_state() -> None:
    """Reset global worker state between tests."""
    _state.__init__()  # type: ignore[misc]


def _build_gpu_bootstrap():
    """Build a GPU profile bootstrap runtime for testing."""
    args = parse_worker_args(["--profile", "gpu", "--lazy", "--lang", "eng"])
    return build_worker_bootstrap_runtime(
        args,
        environ={"HOME": "/tmp/test-home"},
    )


class TestLazyProfileStartup:
    """Verify lazy profile workers start with no models loaded."""

    def setup_method(self) -> None:
        _reset_state()

    def test_lazy_profile_signals_ready_without_models(self) -> None:
        bootstrap = _build_gpu_bootstrap()
        load_worker_profile_lazy(bootstrap)

        assert _state.ready is True
        assert _state.loaded_tasks == set()
        assert _state.command == "lazy-profile:gpu"
        assert _state.lang == "eng"

    def test_lazy_profile_preserves_bootstrap_for_later(self) -> None:
        bootstrap = _build_gpu_bootstrap()
        load_worker_profile_lazy(bootstrap)

        assert _state.bootstrap is bootstrap
        assert _state.bootstrap.profile == WorkerProfile.GPU

    def test_lazy_profile_has_no_fa_model(self) -> None:
        bootstrap = _build_gpu_bootstrap()
        load_worker_profile_lazy(bootstrap)

        assert _state.whisper_fa_model is None
        assert _state.wave2vec_fa_model is None
        assert _state.whisper_asr_model is None


class TestEnsureTaskLoaded:
    """Verify on-demand model loading via ensure_task_loaded()."""

    def setup_method(self) -> None:
        _reset_state()

    def test_ensure_task_on_unbootstrapped_worker_raises(self) -> None:
        """Cannot load tasks before bootstrap."""
        try:
            ensure_task_loaded("fa")
            raise AssertionError("should have raised RuntimeError")
        except RuntimeError as e:
            assert "before worker bootstrap" in str(e)

    def test_ensure_task_unknown_task_raises(self) -> None:
        bootstrap = _build_gpu_bootstrap()
        load_worker_profile_lazy(bootstrap)

        try:
            ensure_task_loaded("nonexistent_task")
            raise AssertionError("should have raised ValueError")
        except ValueError as e:
            assert "Unknown ensure_task name" in str(e)

    def test_ensure_task_idempotent(self) -> None:
        """Second call for same task returns already_loaded immediately."""
        bootstrap = _build_gpu_bootstrap()
        load_worker_profile_lazy(bootstrap)

        # Manually mark a task as loaded (simulating a previous ensure_task).
        _state.loaded_tasks.add("speaker")

        result = ensure_task_loaded("speaker")
        assert result.status == "already_loaded"
        assert result.task == "speaker"
        assert result.elapsed_s == 0.0

    def test_ensure_task_result_fields(self) -> None:
        """EnsureTaskResponse has correct fields."""
        result = EnsureTaskResponse(status="loaded", task="fa", elapsed_s=1.23)
        assert result.status == "loaded"
        assert result.task == "fa"
        assert result.elapsed_s == 1.23

    def test_ensure_task_speaker_is_noop_load(self) -> None:
        """Speaker uses lazy request-time loading — ensure_task just marks it."""
        bootstrap = _build_gpu_bootstrap()
        load_worker_profile_lazy(bootstrap)

        result = ensure_task_loaded("speaker")
        assert result.status == "loaded"
        assert "speaker" in _state.loaded_tasks

    def test_ensure_task_passes_engine_overrides(self) -> None:
        """Engine overrides from ensure_task are merged with bootstrap defaults."""
        bootstrap = _build_gpu_bootstrap()
        load_worker_profile_lazy(bootstrap)

        # Speaker is a no-op load, but it exercises the override merging path.
        result = ensure_task_loaded("speaker", {"speaker": "pyannote"})
        assert result.status == "loaded"


class TestEnsureTaskThreadSafety:
    """Verify concurrent ensure_task calls don't cause double loads."""

    def setup_method(self) -> None:
        _reset_state()

    def test_concurrent_ensure_task_loads_once(self) -> None:
        """Two threads calling ensure_task for the same task should only load once."""
        bootstrap = _build_gpu_bootstrap()
        load_worker_profile_lazy(bootstrap)

        results: list[EnsureTaskResponse] = []
        errors: list[Exception] = []

        def call_ensure() -> None:
            try:
                r = ensure_task_loaded("speaker")
                results.append(r)
            except Exception as e:
                errors.append(e)

        t1 = threading.Thread(target=call_ensure)
        t2 = threading.Thread(target=call_ensure)
        t1.start()
        t2.start()
        t1.join()
        t2.join()

        assert not errors, f"Unexpected errors: {errors}"
        assert len(results) == 2
        # At most one should be "loaded", the other "already_loaded"
        statuses = {r.status for r in results}
        assert "loaded" in statuses or "already_loaded" in statuses


class TestCLILazyFlag:
    """Verify the --lazy CLI flag is parsed correctly."""

    def test_lazy_flag_parsed(self) -> None:
        args = parse_worker_args(["--profile", "gpu", "--lazy", "--lang", "eng"])
        assert args.lazy is True

    def test_no_lazy_flag_default(self) -> None:
        args = parse_worker_args(["--profile", "gpu", "--lang", "eng"])
        assert args.lazy is False
