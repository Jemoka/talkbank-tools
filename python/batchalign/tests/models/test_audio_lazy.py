"""Tests for the injectable lazy-audio loader seam."""

from __future__ import annotations

import torch

from batchalign.inference.audio import ASRAudioFile, set_lazy_audio_enabled


def test_lazy_chunk_uses_partial_load() -> None:
    """Lazy chunk reads should delegate through the injected loader once."""
    calls = {"load": 0}
    seen = {"offset": None, "frames": None}

    def _fake_load(
        _path: str,
        frame_offset: int = 0,
        num_frames: int = -1,
        **_kwargs: object,
    ) -> tuple[torch.Tensor, int]:
        calls["load"] += 1
        seen["offset"] = frame_offset
        seen["frames"] = num_frames
        samples = max(int(num_frames), 0) if num_frames >= 0 else 0
        if samples == 0:
            samples = 10
        return torch.zeros(1, samples), 16000

    audio = ASRAudioFile.lazy("fake.wav", 16000, load_audio_fn=_fake_load)
    chunk = audio.chunk(0, 1000)

    assert calls["load"] == 1
    assert seen["offset"] == 0
    assert seen["frames"] > 0
    assert chunk.numel() > 0


def test_lazy_audio_flag_disables_lazy() -> None:
    """The global feature flag should still hard-disable lazy audio creation."""
    set_lazy_audio_enabled(False)
    try:
        try:
            ASRAudioFile.lazy("fake.wav", 16000)
            assert False, "Expected lazy audio to be disabled"
        except RuntimeError:
            pass
    finally:
        set_lazy_audio_enabled(True)
