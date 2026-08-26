"""Device-policy regressions for the CHATWhisper model loader."""

from types import SimpleNamespace

from batchalign.backends.asr.chatwhisper import (
    _chatwhisper_dtypes,
    _is_mps_device,
    _place_utterance_model,
    _utterance_device,
)
from batchalign.backends.utseg.chatutterance import CHATUtteranceBackend


class _Device:
    def __init__(self, selector: str) -> None:
        self.selector = selector
        self.type = selector.split(":", 1)[0]

    def __str__(self) -> str:
        return self.selector


def _fake_torch(*, cuda: bool = False, mps: bool = False) -> SimpleNamespace:
    return SimpleNamespace(
        device=_Device,
        cuda=SimpleNamespace(is_available=lambda: cuda),
        backends=SimpleNamespace(
            mps=SimpleNamespace(is_available=lambda: mps),
        ),
    )


def test_mps_device_selectors_are_detected_before_model_construction() -> None:
    assert _is_mps_device("mps")
    assert _is_mps_device("MPS:0")


def test_non_mps_device_selectors_keep_existing_dtype_fallback() -> None:
    assert not _is_mps_device(None)
    assert not _is_mps_device("cpu")
    assert not _is_mps_device("cuda:0")


def test_mps_uses_only_float32_instead_of_bfloat16() -> None:
    torch = SimpleNamespace(float32="float32", bfloat16="bfloat16", float16="float16")
    assert _chatwhisper_dtypes(torch, "mps") == ("float32",)
    assert _chatwhisper_dtypes(torch, "mps:0") == ("float32",)


def test_cpu_and_cuda_keep_bfloat16_then_float16_fallback() -> None:
    torch = SimpleNamespace(float32="float32", bfloat16="bfloat16", float16="float16")
    expected = ("bfloat16", "float16")
    assert _chatwhisper_dtypes(torch, None) == expected
    assert _chatwhisper_dtypes(torch, "cpu") == expected
    assert _chatwhisper_dtypes(torch, "cuda:0") == expected


def test_utterance_segmenter_uses_requested_mps_when_available() -> None:
    device = _utterance_device(_fake_torch(mps=True), "mps")
    assert str(device) == "mps"


def test_utterance_segmenter_falls_back_when_mps_is_unavailable(caplog) -> None:
    device = _utterance_device(_fake_torch(mps=False), "mps")
    assert str(device) == "cpu"
    assert "falling back to CPU" in caplog.text


def test_utterance_segmenter_falls_back_when_mps_model_move_fails() -> None:
    moves: list[str] = []

    class Model:
        def to(self, device):
            moves.append(str(device))
            if device.type == "mps":
                raise RuntimeError("MPS allocation failed")
            return self

    model, device = _place_utterance_model(
        _fake_torch(mps=True),
        Model(),
        "mps",
    )

    assert isinstance(model, Model)
    assert str(device) == "cpu"
    assert moves == ["mps", "cpu"]


def test_mps_utterance_results_use_a_separate_cache_namespace() -> None:
    segmenter = SimpleNamespace(device=_Device("mps"))
    backend = CHATUtteranceBackend(segmenter=segmenter)
    assert backend.name.endswith(":mps")


def test_cpu_utterance_results_keep_the_existing_cache_namespace() -> None:
    segmenter = SimpleNamespace(device=_Device("cpu"))
    backend = CHATUtteranceBackend(segmenter=segmenter)
    assert backend.name == "chatutterance:talkbank/CHATUtterance-en:typed2"
