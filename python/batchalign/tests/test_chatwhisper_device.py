"""Device-policy regressions for the CHATWhisper model loader."""

from types import SimpleNamespace

from batchalign.backends.asr.chatwhisper import (
    _chatwhisper_dtypes,
    _is_mps_device,
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
