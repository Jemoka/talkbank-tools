"""Runtime seam tests for the Aliyun HK ASR transport helper."""

from __future__ import annotations

import builtins
import configparser
from types import ModuleType, SimpleNamespace
import sys
import wave

import pytest
import torch

from batchalign.inference.asr import AsrElement, AsrMonologue, MonologueAsrResponse
from batchalign.inference.languages.cantonese._aliyun_asr import (
    _AliyunRunner,
    _ensure_wav,
    _get_token,
)
import batchalign.inference.languages.cantonese._aliyun_asr as aliyun_asr


def _config_with_asr(**entries: str) -> configparser.ConfigParser:
    cfg = configparser.ConfigParser()
    cfg.add_section("asr")
    for key, value in entries.items():
        cfg.set("asr", key, value)
    return cfg


def test_runner_start_reports_missing_sdk_dependency(monkeypatch) -> None:
    original_import = builtins.__import__

    def fake_import(name, *args, **kwargs):
        if name == "nls":
            raise ImportError("missing")
        return original_import(name, *args, **kwargs)

    monkeypatch.setattr(builtins, "__import__", fake_import)

    with pytest.raises(ImportError, match="Aliyun engine dependencies"):
        _AliyunRunner(token="token", appkey="app", wav_path="clip.wav").start()


def test_runner_loads_pcm_bytes_from_wav(tmp_path) -> None:
    wav_path = tmp_path / "clip.wav"
    with wave.open(str(wav_path), "wb") as wav_file:
        wav_file.setnchannels(1)
        wav_file.setsampwidth(2)
        wav_file.setframerate(8000)
        wav_file.writeframes(b"\x01\x00\x02\x00")

    runner = _AliyunRunner(token="token", appkey="app", wav_path=str(wav_path))
    runner._load_file()

    assert runner._audio_data == b"\x01\x00\x02\x00"
    assert runner._sample_rate == 8000


def test_runner_noop_callbacks() -> None:
    runner = _AliyunRunner(token="token", appkey="app", wav_path="clip.wav")
    runner._on_start("start")
    runner._on_result_changed("changed")
    runner._on_completed("done")


def test_runner_start_streams_audio_chunks(monkeypatch) -> None:
    events: dict[str, object] = {"chunks": []}

    class FakeTranscriber:
        def __init__(self, **kwargs) -> None:
            events["callbacks"] = kwargs

        def start(self, **kwargs) -> None:
            events["start_kwargs"] = kwargs

        def send_audio(self, chunk: bytes) -> None:
            events["chunks"].append(chunk)

        def ctrl(self, **kwargs) -> None:
            events["ctrl_kwargs"] = kwargs

        def stop(self) -> None:
            events["stopped"] = True

    fake_nls = ModuleType("nls")
    fake_nls.NlsSpeechTranscriber = FakeTranscriber
    monkeypatch.setitem(sys.modules, "nls", fake_nls)
    monkeypatch.setattr(
        _AliyunRunner,
        "_load_file",
        lambda self: (
            setattr(self, "_audio_data", b"a" * 800),
            setattr(self, "_sample_rate", 8000),
        ),
    )
    monkeypatch.setattr("batchalign.inference.languages.cantonese._aliyun_asr.time.sleep", lambda _s: None)

    runner = _AliyunRunner(token="token", appkey="app", wav_path="clip.wav")
    result = runner.start()

    assert result == []
    assert events["start_kwargs"]["sample_rate"] == 8000
    assert events["chunks"] == [b"a" * 640, b"a" * 160]
    assert events["ctrl_kwargs"] == {"ex": {"source": "batchalign3"}}
    assert events["stopped"] is True


def test_get_token_reports_missing_sdk_dependency(monkeypatch) -> None:
    original_import = builtins.__import__
    monkeypatch.setattr(aliyun_asr, "_cached_token", None)
    monkeypatch.setattr(aliyun_asr, "_cached_token_time", 0.0)

    def fake_import(name, *args, **kwargs):
        if name.startswith("aliyunsdkcore"):
            raise ImportError("missing")
        return original_import(name, *args, **kwargs)

    monkeypatch.setattr(builtins, "__import__", fake_import)

    with pytest.raises(ImportError, match="Aliyun engine dependencies"):
        _get_token("id", "secret")


def test_get_token_uses_cached_value(monkeypatch) -> None:
    monkeypatch.setattr(aliyun_asr, "_cached_token", "cached")
    monkeypatch.setattr(aliyun_asr, "_cached_token_time", 10.0)
    monkeypatch.setattr("batchalign.inference.languages.cantonese._aliyun_asr.time.monotonic", lambda: 11.0)

    assert _get_token("id", "secret") == "cached"


def test_get_token_requires_token_id(monkeypatch) -> None:
    client_module = ModuleType("aliyunsdkcore.client")

    class AcsClient:
        def __init__(self, *_args) -> None:
            pass

        def do_action_with_exception(self, _request):
            return '{"Token": {}}'

    client_module.AcsClient = AcsClient

    request_module = ModuleType("aliyunsdkcore.request")

    class CommonRequest:
        def set_method(self, _method: str) -> None:
            return None

        def set_domain(self, _domain: str) -> None:
            return None

        def set_version(self, _version: str) -> None:
            return None

        def set_action_name(self, _action: str) -> None:
            return None

    request_module.CommonRequest = CommonRequest

    monkeypatch.setitem(sys.modules, "aliyunsdkcore", ModuleType("aliyunsdkcore"))
    monkeypatch.setitem(sys.modules, "aliyunsdkcore.client", client_module)
    monkeypatch.setitem(sys.modules, "aliyunsdkcore.request", request_module)
    monkeypatch.setattr(aliyun_asr, "_cached_token", None)
    monkeypatch.setattr(aliyun_asr, "_cached_token_time", 0.0)
    monkeypatch.setattr("batchalign.inference.languages.cantonese._aliyun_asr.time.monotonic", lambda: 1.0)

    with pytest.raises(RuntimeError, match="Token.Id"):
        _get_token("id", "secret")


def test_get_token_refreshes_and_caches_value(monkeypatch) -> None:
    client_module = ModuleType("aliyunsdkcore.client")

    class AcsClient:
        def __init__(self, *_args) -> None:
            pass

        def do_action_with_exception(self, _request):
            return '{"Token": {"Id": "fresh-token"}}'

    client_module.AcsClient = AcsClient

    request_module = ModuleType("aliyunsdkcore.request")

    class CommonRequest:
        def set_method(self, _method: str) -> None:
            return None

        def set_domain(self, _domain: str) -> None:
            return None

        def set_version(self, _version: str) -> None:
            return None

        def set_action_name(self, _action: str) -> None:
            return None

    request_module.CommonRequest = CommonRequest

    monkeypatch.setitem(sys.modules, "aliyunsdkcore", ModuleType("aliyunsdkcore"))
    monkeypatch.setitem(sys.modules, "aliyunsdkcore.client", client_module)
    monkeypatch.setitem(sys.modules, "aliyunsdkcore.request", request_module)
    monkeypatch.setattr(aliyun_asr, "_cached_token", "")
    monkeypatch.setattr(aliyun_asr, "_cached_token_time", 0.0)
    monkeypatch.setattr("batchalign.inference.languages.cantonese._aliyun_asr.time.monotonic", lambda: 100.0)

    token = _get_token("id", "secret")

    assert token == "fresh-token"
    assert aliyun_asr._cached_token == "fresh-token"
    assert aliyun_asr._cached_token_time == 100.0


def test_ensure_wav_converts_non_wav_audio(monkeypatch) -> None:
    saved: dict[str, object] = {}
    monkeypatch.setattr(
        "batchalign.inference.audio.load_audio",
        lambda _path: (
            torch.tensor([[1.0, 3.0], [2.0, 4.0]], dtype=torch.float32),
            16000,
        ),
    )
    monkeypatch.setattr(
        "batchalign.inference.audio.save_audio",
        lambda path, audio, sample_rate, bits_per_sample=16: saved.update(
            {
                "path": str(path),
                "audio": audio.clone(),
                "sample_rate": sample_rate,
                "bits_per_sample": bits_per_sample,
            }
        ),
    )

    wav_path, temp_dir = _ensure_wav("clip.mp3")
    try:
        assert temp_dir is not None
        assert wav_path.endswith(".wav")
        assert saved["path"].endswith(".wav")
        assert saved["bits_per_sample"] == 16
        assert saved["sample_rate"] == 16000
        assert saved["audio"].shape == (1, 2)
    finally:
        if temp_dir is not None:
            temp_dir.cleanup()


def test_ensure_wav_returns_existing_wav_path() -> None:
    wav_path, temp_dir = _ensure_wav("clip.wav")

    assert wav_path == "clip.wav"
    assert temp_dir is None


def test_transcribe_to_monologues_cleans_up_temp_dir(monkeypatch) -> None:
    cleanup_called = {"value": False}

    class _TempDir:
        def cleanup(self) -> None:
            cleanup_called["value"] = True

    class FakeRunner:
        def __init__(self, *, token: str, appkey: str, wav_path: str) -> None:
            self.token = token
            self.appkey = appkey
            self.wav_path = wav_path

        def start(self):
            return ["raw-result"]

    expected = MonologueAsrResponse(
        lang="yue",
        monologues=[
            AsrMonologue(
                speaker=0,
                elements=[AsrElement(value="好", ts=0.0, end_ts=0.2, type="text")],
            )
        ],
    )

    monkeypatch.setattr(aliyun_asr, "_ak_id", "id")
    monkeypatch.setattr(aliyun_asr, "_ak_secret", "secret")
    monkeypatch.setattr(aliyun_asr, "_appkey", "app")
    monkeypatch.setattr(aliyun_asr, "_get_token", lambda *_args: "token")
    monkeypatch.setattr(aliyun_asr, "_ensure_wav", lambda _path: ("clip.wav", _TempDir()))
    monkeypatch.setattr(aliyun_asr, "_AliyunRunner", FakeRunner)
    monkeypatch.setattr(aliyun_asr, "_project_results", lambda _results: expected)

    response = aliyun_asr._transcribe_to_monologues("clip.mp3")

    assert response == expected
    assert cleanup_called["value"] is True


def test_load_aliyun_asr_stores_credentials() -> None:
    aliyun_asr.load_aliyun_asr(
        "yue",
        None,
        config=_config_with_asr(
            **{
                "engine.aliyun.ak_id": "id",
                "engine.aliyun.ak_secret": "secret",
                "engine.aliyun.ak_appkey": "appkey",
            }
        ),
    )

    assert aliyun_asr._ak_id == "id"
    assert aliyun_asr._ak_secret == "secret"
    assert aliyun_asr._appkey == "appkey"
