"""Runtime seam tests for the Tencent HK ASR transport helper."""

from __future__ import annotations

import builtins
import configparser
from types import ModuleType, SimpleNamespace
import sys

import pytest

from batchalign.errors import ConfigError
from batchalign.inference.languages.cantonese._tencent_api import TencentRecognizer


def _config_with_asr(**entries: str) -> configparser.ConfigParser:
    cfg = configparser.ConfigParser()
    cfg.add_section("asr")
    for key, value in entries.items():
        cfg.set("asr", key, value)
    return cfg


def _install_tencent_init_modules(monkeypatch) -> None:
    qcloud_cos = ModuleType("qcloud_cos")

    class CosConfig:
        def __init__(self, **kwargs) -> None:
            self.kwargs = kwargs

    class CosS3Client:
        def __init__(self, config) -> None:
            self.config = config

    qcloud_cos.CosConfig = CosConfig
    qcloud_cos.CosS3Client = CosS3Client

    asr_client_module = ModuleType("tencentcloud.asr.v20190614.asr_client")

    class AsrClient:
        def __init__(self, credential, region: str) -> None:
            self.credential = credential
            self.region = region

    asr_client_module.AsrClient = AsrClient

    credential_module = ModuleType("tencentcloud.common.credential")

    class Credential:
        def __init__(self, secret_id: str, secret_key: str) -> None:
            self.secret_id = secret_id
            self.secret_key = secret_key

    credential_module.Credential = Credential

    monkeypatch.setitem(sys.modules, "qcloud_cos", qcloud_cos)
    monkeypatch.setitem(sys.modules, "tencentcloud", ModuleType("tencentcloud"))
    monkeypatch.setitem(sys.modules, "tencentcloud.asr", ModuleType("tencentcloud.asr"))
    monkeypatch.setitem(
        sys.modules,
        "tencentcloud.asr.v20190614",
        ModuleType("tencentcloud.asr.v20190614"),
    )
    monkeypatch.setitem(
        sys.modules,
        "tencentcloud.asr.v20190614.asr_client",
        asr_client_module,
    )
    monkeypatch.setitem(sys.modules, "tencentcloud.common", ModuleType("tencentcloud.common"))
    monkeypatch.setitem(sys.modules, "tencentcloud.common.credential", credential_module)


def _valid_config(**overrides: str) -> configparser.ConfigParser:
    base = {
        "engine.tencent.id": "id",
        "engine.tencent.key": "key",
        "engine.tencent.region": "ap-guangzhou",
        "engine.tencent.bucket": "bucket",
    }
    base.update(overrides)
    return _config_with_asr(**base)


def _make_recognizer() -> TencentRecognizer:
    recognizer = TencentRecognizer.__new__(TencentRecognizer)
    recognizer.lang_code = "yue"
    recognizer.provider_lang = "yue"
    recognizer._poll_interval_s = 0.0
    recognizer._bucket_name = "bucket"
    recognizer._region = "ap-guangzhou"
    return recognizer


def test_init_reports_missing_sdk_dependency(monkeypatch) -> None:
    original_import = builtins.__import__

    def fake_import(name, *args, **kwargs):
        if name == "qcloud_cos" or name.startswith("tencentcloud"):
            raise ImportError("missing")
        return original_import(name, *args, **kwargs)

    monkeypatch.setattr(builtins, "__import__", fake_import)

    with pytest.raises(ImportError, match="Tencent engine dependencies"):
        TencentRecognizer("yue", config=_valid_config())


def test_init_wires_sdk_clients_and_language_state(monkeypatch) -> None:
    _install_tencent_init_modules(monkeypatch)

    recognizer = TencentRecognizer("yue", poll_interval_s=0.5, config=_valid_config())

    assert recognizer.lang_code == "yue"
    assert recognizer.provider_lang == "yue"
    assert recognizer._poll_interval_s == 1.0
    assert recognizer._bucket_name == "bucket"
    assert recognizer._region == "ap-guangzhou"


def test_init_rejects_empty_region_in_shared_config_reader(monkeypatch) -> None:
    _install_tencent_init_modules(monkeypatch)

    with pytest.raises(ConfigError, match="engine.tencent.region"):
        TencentRecognizer("yue", config=_valid_config(**{"engine.tencent.region": ""}))


def test_transcribe_reports_missing_runtime_sdk_dependency(monkeypatch) -> None:
    original_import = builtins.__import__

    def fake_import(name, *args, **kwargs):
        if name == "tencentcloud.asr.v20190614":
            raise ImportError("missing")
        return original_import(name, *args, **kwargs)

    monkeypatch.setattr(builtins, "__import__", fake_import)

    with pytest.raises(ImportError, match="Tencent engine dependencies"):
        _make_recognizer().transcribe("clip.wav")


def test_transcribe_surfaces_failed_status_and_ignores_cleanup_errors(monkeypatch) -> None:
    models = SimpleNamespace(
        CreateRecTaskRequest=type("CreateRecTaskRequest", (), {}),
        DescribeTaskStatusRequest=type("DescribeTaskStatusRequest", (), {}),
    )
    module = ModuleType("tencentcloud.asr.v20190614")
    module.models = models
    monkeypatch.setitem(sys.modules, "tencentcloud.asr.v20190614", module)

    class FakeBucket:
        def upload_file(self, **_kwargs) -> None:
            return None

        def delete_object(self, **_kwargs) -> None:
            raise RuntimeError("cleanup failed")

    class FakeAsrClient:
        def CreateRecTask(self, _request):
            return SimpleNamespace(Data=SimpleNamespace(TaskId=123))

        def DescribeTaskStatus(self, _request):
            return SimpleNamespace(Data=SimpleNamespace(Status=3, ErrorMsg="bad result"))

    recognizer = _make_recognizer()
    recognizer._bucket = FakeBucket()
    recognizer._asr_client = FakeAsrClient()

    with pytest.raises(RuntimeError, match="Tencent ASR failed: bad result"):
        recognizer.transcribe("clip.wav")


def test_transcribe_returns_result_detail_after_polling(monkeypatch) -> None:
    models = SimpleNamespace(
        CreateRecTaskRequest=type("CreateRecTaskRequest", (), {}),
        DescribeTaskStatusRequest=type("DescribeTaskStatusRequest", (), {}),
    )
    module = ModuleType("tencentcloud.asr.v20190614")
    module.models = models
    monkeypatch.setitem(sys.modules, "tencentcloud.asr.v20190614", module)

    class FakeBucket:
        def __init__(self) -> None:
            self.deleted: list[dict[str, object]] = []

        def upload_file(self, **_kwargs) -> None:
            return None

        def delete_object(self, **kwargs) -> None:
            self.deleted.append(kwargs)

    class FakeAsrClient:
        def __init__(self) -> None:
            self.calls = 0

        def CreateRecTask(self, request):
            self.request = request
            return SimpleNamespace(Data=SimpleNamespace(TaskId=123))

        def DescribeTaskStatus(self, _request):
            self.calls += 1
            if self.calls == 1:
                return SimpleNamespace(Data=SimpleNamespace(Status=1))
            return SimpleNamespace(
                Data=SimpleNamespace(Status=2, ResultDetail=[{"word": "好"}])
            )

    monotonic_values = iter([0.0, 1.0])
    sleep_calls: list[float] = []
    monkeypatch.setattr(
        "batchalign.inference.languages.cantonese._tencent_api.time.monotonic",
        lambda: next(monotonic_values),
    )
    monkeypatch.setattr(
        "batchalign.inference.languages.cantonese._tencent_api.time.sleep",
        lambda seconds: sleep_calls.append(seconds),
    )

    recognizer = _make_recognizer()
    recognizer._bucket = bucket = FakeBucket()
    recognizer._asr_client = FakeAsrClient()

    result = recognizer.transcribe("clip.wav", num_speakers=2)

    assert result == [{"word": "好"}]
    assert sleep_calls == [0.0]
    assert bucket.deleted[0]["Bucket"] == "bucket"


def test_transcribe_times_out_when_status_never_completes(monkeypatch) -> None:
    models = SimpleNamespace(
        CreateRecTaskRequest=type("CreateRecTaskRequest", (), {}),
        DescribeTaskStatusRequest=type("DescribeTaskStatusRequest", (), {}),
    )
    module = ModuleType("tencentcloud.asr.v20190614")
    module.models = models
    monkeypatch.setitem(sys.modules, "tencentcloud.asr.v20190614", module)

    class FakeBucket:
        def upload_file(self, **_kwargs) -> None:
            return None

        def delete_object(self, **_kwargs) -> None:
            return None

    class FakeAsrClient:
        def CreateRecTask(self, _request):
            return SimpleNamespace(Data=SimpleNamespace(TaskId=123))

        def DescribeTaskStatus(self, _request):
            return SimpleNamespace(Data=SimpleNamespace(Status=1))

    monotonic_values = iter([0.0, 601.0])
    monkeypatch.setattr(
        "batchalign.inference.languages.cantonese._tencent_api.time.monotonic",
        lambda: next(monotonic_values),
    )
    monkeypatch.setattr("batchalign.inference.languages.cantonese._tencent_api.time.sleep", lambda _s: None)

    recognizer = _make_recognizer()
    recognizer._bucket = FakeBucket()
    recognizer._asr_client = FakeAsrClient()

    with pytest.raises(RuntimeError, match="timed out"):
        recognizer.transcribe("clip.wav")
