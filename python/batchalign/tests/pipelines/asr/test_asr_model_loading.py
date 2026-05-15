"""Tests for ASR worker bootstrap configuration helpers.

These tests keep the server/worker credential boundary explicit without
reaching for monkeypatching. The helpers accept plain mappings so the policy
can be checked directly.
"""

from __future__ import annotations

from batchalign.device import DevicePolicy
from batchalign.worker._model_loading.asr import (
    load_asr_engine,
    resolve_asr_engine,
    resolve_injected_revai_api_key,
)
from batchalign.worker._types import AsrEngine, InferTask, WorkerBootstrapRuntime, _state


class TestResolveInjectedRevaiApiKey:
    """Credential normalization stays at the explicit runtime boundary."""

    def test_empty_values_resolve_to_none(self) -> None:
        env = {"BATCHALIGN_REV_API_KEY": "   "}
        assert resolve_injected_revai_api_key(env) is None

    def test_injected_env_helper_normalizes_supported_keys(self) -> None:
        env = {"REVAI_API_KEY": "  from-env-helper "}
        assert resolve_injected_revai_api_key(env) == "from-env-helper"


class TestResolveAsrEngine:
    """Engine selection must stay deterministic and type-light."""

    def test_override_wins(self) -> None:
        assert resolve_asr_engine({"asr": "whisper"}, "from-config") == "whisper"

    def test_rev_is_selected_when_key_present(self) -> None:
        assert resolve_asr_engine(None, "from-config") == "rev"

    def test_whisper_is_default_without_key(self) -> None:
        assert resolve_asr_engine(None, None) == "whisper"

    def test_whisper_hub_override_wins(self) -> None:
        assert (
            resolve_asr_engine({"asr": "whisper_hub"}, "from-config") == "whisper_hub"
        )


def test_whisper_override_does_not_require_legacy_config(monkeypatch) -> None:
    """Whisper bootstrap should not touch legacy config-discovery paths."""

    def fake_load_whisper_asr(*, language, device_policy):
        assert language == "english"
        assert device_policy == DevicePolicy(force_cpu=True)
        return "fake-whisper-model"

    old_model = _state.whisper_asr_model
    old_engine = _state.asr_engine
    old_key = _state.rev_api_key
    try:
        monkeypatch.setattr(
            "batchalign.inference.asr.load_whisper_asr",
            fake_load_whisper_asr,
        )

        load_asr_engine(
            WorkerBootstrapRuntime(
                task=InferTask.ASR,
                lang="eng",
                num_speakers=1,
                engine_overrides={"asr": "whisper"},
                device_policy=DevicePolicy(force_cpu=True),
            )
        )

        assert _state.whisper_asr_model == "fake-whisper-model"
        assert _state.asr_engine is AsrEngine.WHISPER
    finally:
        _state.whisper_asr_model = old_model
        _state.asr_engine = old_engine
        _state.rev_api_key = old_key


def test_injected_revai_key_selects_rev_without_legacy_config() -> None:
    """Rev.AI selection should rely on the Rust-injected key, not config rediscovery."""

    old_engine = _state.asr_engine
    old_key = _state.rev_api_key
    try:
        load_asr_engine(
            WorkerBootstrapRuntime(
                task=InferTask.ASR,
                lang="eng",
                num_speakers=1,
                revai_api_key=" from-rust ",
            )
        )

        assert _state.asr_engine is AsrEngine.REV
        assert _state.rev_api_key == " from-rust "
    finally:
        _state.asr_engine = old_engine
        _state.rev_api_key = old_key


def test_whisper_hub_override_dispatches_to_whisper_hub_loader(monkeypatch) -> None:
    """Worker dispatch must route asr=whisper_hub to the fine-tune loader.

    RED when ``AsrEngine.WHISPER_HUB`` does not yet exist and when
    ``_model_loading.asr.load_asr_engine`` has no whisper_hub branch.
    """

    captured: dict[str, object] = {}
    old_engine = _state.asr_engine
    old_model = _state.whisper_asr_model
    try:
        def fake_load_whisper_hub_asr(lang, engine_overrides, *, device_policy):
            captured["lang"] = lang
            captured["engine_overrides"] = engine_overrides
            captured["device_policy"] = device_policy
            return "fake-whisper-hub-model"

        monkeypatch.setattr(
            "batchalign.inference.whisper_hub.load_whisper_hub_asr",
            fake_load_whisper_hub_asr,
        )

        load_asr_engine(
            WorkerBootstrapRuntime(
                task=InferTask.ASR,
                lang="mal",
                num_speakers=1,
                engine_overrides={"asr": "whisper_hub"},
                device_policy=DevicePolicy(force_cpu=True),
            )
        )

        assert captured["lang"] == "mal"
        assert captured["engine_overrides"] == {"asr": "whisper_hub"}
        assert captured["device_policy"] == DevicePolicy(force_cpu=True)
        assert _state.whisper_asr_model == "fake-whisper-hub-model"
        assert _state.asr_engine is AsrEngine.WHISPER_HUB
    finally:
        _state.asr_engine = old_engine
        _state.whisper_asr_model = old_model


def test_tencent_override_uses_injected_boundary_credentials(monkeypatch) -> None:
    """HK bootstrap should delegate credential discovery to the Rust boundary."""

    captured: dict[str, object] = {}
    old_engine = _state.asr_engine
    try:
        def fake_load_tencent_asr(lang, engine_overrides, *, config=None):
            captured["lang"] = lang
            captured["engine_overrides"] = engine_overrides
            captured["config"] = config

        monkeypatch.setattr(
            "batchalign.inference.languages.cantonese._tencent_asr.load_tencent_asr",
            fake_load_tencent_asr,
        )

        load_asr_engine(
            WorkerBootstrapRuntime(
                task=InferTask.ASR,
                lang="yue",
                num_speakers=1,
                engine_overrides={"asr": "tencent"},
            )
        )

        assert captured == {
            "lang": "yue",
            "engine_overrides": {"asr": "tencent"},
            "config": None,
        }
        assert _state.asr_engine is AsrEngine.TENCENT
    finally:
        _state.asr_engine = old_engine
