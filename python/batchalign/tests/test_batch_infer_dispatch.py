# affects: python/batchalign/worker/_execute_v2.py
# affects: python/batchalign/inference/**
"""Tests for the shrinking legacy ``batch_infer`` dispatch boundary.

The Python cutover now requires `opensmile`, `avqi`, and `speaker` to use the
typed V2 execute path. This suite keeps the legacy router honest by proving the
remaining dynamic handlers still work while migrated tasks fail explicitly.
"""

from __future__ import annotations

import importlib

from batchalign.worker._types import (
    BatchInferRequest,
    BatchInferResponse,
    InferResponse,
    InferTask,
    _state,
)
from batchalign.worker._infer import _batch_infer


# ---------------------------------------------------------------------------
# InferTask enum coverage
# ---------------------------------------------------------------------------


class TestInferTaskEnum:
    """Verify the new InferTask variants exist and serialize correctly."""

    def test_opensmile_variant(self) -> None:
        assert InferTask.OPENSMILE.value == "opensmile"

    def test_avqi_variant(self) -> None:
        assert InferTask.AVQI.value == "avqi"

    def test_speaker_variant(self) -> None:
        assert InferTask.SPEAKER.value == "speaker"

    # Benchmark is intentionally absent here: WER scoring is Rust-only utility
    # code now, not a worker infer task.


class TestBatchItemValidation:
    """Verify Pydantic models validate batch items correctly."""

    def test_opensmile_batch_item_valid(self) -> None:
        from batchalign.inference.opensmile import OpenSmileBatchItem

        item = OpenSmileBatchItem(audio_path="/tmp/test.wav")
        assert item.audio_path == "/tmp/test.wav"

    def test_avqi_batch_item_valid(self) -> None:
        from batchalign.inference.avqi import AvqiBatchItem

        item = AvqiBatchItem(cs_file="/tmp/cs.wav", sv_file="/tmp/sv.wav")
        assert item.cs_file == "/tmp/cs.wav"
        assert item.sv_file == "/tmp/sv.wav"

# ---------------------------------------------------------------------------
# Dispatch routing (test-echo disabled — hits real dispatch)
# ---------------------------------------------------------------------------


class TestDispatchRouting:
    """Verify _batch_infer routes to correct handlers and handles errors."""

    def test_opensmile_is_not_routed_through_batch_infer_any_more(self) -> None:
        """openSMILE should now fail fast on the legacy batch-infer path."""
        req = BatchInferRequest(
            task=InferTask.OPENSMILE,
            lang="eng",
            items=[{"bad_field": "not_valid"}],
        )
        resp = _batch_infer(req)
        assert len(resp.results) == 1
        assert resp.results[0].error == f"Unknown task: {InferTask.OPENSMILE}"

    def test_avqi_is_not_routed_through_batch_infer_any_more(self) -> None:
        """AVQI should now fail fast on the legacy batch-infer path."""
        req = BatchInferRequest(
            task=InferTask.AVQI,
            lang="eng",
            items=[{"missing": "fields"}],
        )
        resp = _batch_infer(req)
        assert len(resp.results) == 1
        assert resp.results[0].error == f"Unknown task: {InferTask.AVQI}"

    def test_speaker_is_not_routed_through_batch_infer_any_more(self) -> None:
        req = BatchInferRequest(
            task=InferTask.SPEAKER,
            lang="eng",
            items=[{}],
        )
        resp = _batch_infer(req)
        assert len(resp.results) == 1
        assert resp.results[0].error == f"Unknown task: {InferTask.SPEAKER}"

    def test_empty_batch_returns_empty(self) -> None:
        """Empty batch should return empty results."""
        for task in [InferTask.OPENSMILE, InferTask.AVQI]:
            req = BatchInferRequest(task=task, lang="eng", items=[])
            resp = _batch_infer(req)
            assert len(resp.results) == 0

    def test_bootstrap_registered_handler_routes_dynamic_task(self) -> None:
        """Dynamic tasks should route through the bootstrap-installed handler table."""
        previous_handlers = dict(_state.batch_infer_handlers)

        def _fake_translate(req: BatchInferRequest) -> BatchInferResponse:
            results: list[InferResponse] = []
            for raw_item in req.items:
                assert isinstance(raw_item, dict)
                results.append(InferResponse(
                    result={"raw_translation": str(raw_item["text"]).upper()},
                    elapsed_s=0.0,
                ))
            return BatchInferResponse(results=results)

        _state.clear_batch_infer_handlers()
        _state.register_batch_infer_handler(InferTask.TRANSLATE, _fake_translate)
        try:
            req = BatchInferRequest(
                task=InferTask.TRANSLATE,
                lang="eng",
                items=[{"text": "hello"}],
            )
            resp = _batch_infer(req)
            assert len(resp.results) == 1
            assert resp.results[0].error is None
            assert resp.results[0].result == {"raw_translation": "HELLO"}
        finally:
            _state.clear_batch_infer_handlers()
            _state.batch_infer_handlers.update(previous_handlers)

    def test_asr_is_not_routed_through_batch_infer_any_more(self) -> None:
        """ASR should now fail fast on the legacy batch-infer path.

        The live worker uses ``execute_v2(task="asr")`` instead. Keeping this
        assertion explicit prevents the old ASR request bag from silently
        re-entering the worker bootstrap path later.
        """

        previous_handlers = dict(_state.batch_infer_handlers)
        _state.clear_batch_infer_handlers()
        try:
            req = BatchInferRequest(
                task=InferTask.ASR,
                lang="eng",
                items=[{"audio_path": "/tmp/example.wav"}],
            )
            resp = _batch_infer(req)
            assert len(resp.results) == 1
            assert resp.results[0].error == f"Unknown task: {InferTask.ASR}"
        finally:
            _state.clear_batch_infer_handlers()
            _state.batch_infer_handlers.update(previous_handlers)


# ---------------------------------------------------------------------------
# Capabilities advertisement
# ---------------------------------------------------------------------------


class TestCapabilitiesAdvertisement:
    """Verify _capabilities() includes new infer tasks."""

    def test_capabilities_import_gated_tasks(self) -> None:
        """Tasks with importable deps are advertised; others are excluded."""
        from batchalign.worker._types import _state

        old_test_echo = _state.test_echo
        old_ready = _state.ready
        _state.test_echo = False
        _state.ready = True
        try:
            from batchalign.worker._handlers import _capabilities

            caps = _capabilities()
            # Stanza is always installed in dev → morphosyntax, utseg, coref
            assert InferTask.MORPHOSYNTAX in caps.infer_tasks
            assert InferTask.UTSEG in caps.infer_tasks
            assert InferTask.COREF in caps.infer_tasks
            # Tasks gated on optional deps: only present if installed
            # (no assertion on OPENSMILE, AVQI, SPEAKER, ASR — may or may not be installed)
        finally:
            _state.test_echo = old_test_echo
            _state.ready = old_ready

    def test_capabilities_uses_injected_revai_key_without_config_read(self, monkeypatch) -> None:
        """ASR capability should come from the injected bootstrap key, not config discovery."""
        import batchalign.config as legacy_config
        from batchalign.worker._handlers import _capabilities
        from batchalign.worker._types import WorkerBootstrapRuntime

        original_import_module = importlib.import_module

        def fake_import_module(name: str, package=None):
            if name == "whisper":
                raise ModuleNotFoundError(name)
            return original_import_module(name, package)

        def fail_config_read(*_args, **_kwargs):
            raise AssertionError("config_read should not run for capability detection")

        old_test_echo = _state.test_echo
        old_ready = _state.ready
        old_bootstrap = _state.bootstrap
        old_asr_engine = _state.asr_engine
        try:
            monkeypatch.setattr(importlib, "import_module", fake_import_module)
            monkeypatch.setattr(legacy_config, "config_read", fail_config_read)
            _state.test_echo = False
            _state.ready = False
            _state.bootstrap = WorkerBootstrapRuntime(
                task=None,
                lang="eng",
                num_speakers=1,
                revai_api_key="from-rust",
            )

            caps = _capabilities()
            assert InferTask.ASR in caps.infer_tasks
            assert caps.engine_versions[InferTask.ASR] == "rev"
        finally:
            _state.test_echo = old_test_echo
            _state.ready = old_ready
            _state.bootstrap = old_bootstrap
            _state.asr_engine = old_asr_engine
