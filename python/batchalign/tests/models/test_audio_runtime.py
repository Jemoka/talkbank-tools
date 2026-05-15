"""Focused tests for lazy audio fallbacks and Whisper timestamp extraction."""

from __future__ import annotations

from pathlib import Path
from types import ModuleType, SimpleNamespace
import sys

import numpy as np
import pytest
import torch

from batchalign.inference.audio import (
    ASRAudioFile,
    _extract_token_timestamps,
    audio_info,
    load_audio,
    load_audio_file,
)


TEST_AUDIO = Path(__file__).parent.parent / "support" / "test.mp3"


def _install_fake_resample(
    monkeypatch,
    *,
    result: torch.Tensor,
    calls: list[tuple[int, int]],
    seen_inputs: list[torch.Tensor] | None = None,
) -> None:
    transforms = ModuleType("torchaudio.transforms")

    class _Resample:
        def __init__(self, source_rate: int, target_rate: int) -> None:
            calls.append((source_rate, target_rate))

        def __call__(self, audio: torch.Tensor) -> torch.Tensor:
            if seen_inputs is not None:
                seen_inputs.append(audio.clone())
            return result.clone()

    transforms.Resample = _Resample
    torchaudio = ModuleType("torchaudio")
    torchaudio.transforms = transforms
    monkeypatch.setitem(sys.modules, "torchaudio", torchaudio)
    monkeypatch.setitem(sys.modules, "torchaudio.transforms", transforms)


def _install_whisper_generation_helpers(
    monkeypatch,
    *,
    recorded_matrices: list[np.ndarray],
) -> None:
    module = ModuleType("transformers.models.whisper.generation_whisper")

    def _dynamic_time_warping(matrix: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
        recorded_matrices.append(matrix)
        text_len = matrix.shape[0]
        return (
            np.arange(text_len, dtype=np.int64),
            np.arange(1, text_len + 1, dtype=np.int64),
        )

    module._dynamic_time_warping = _dynamic_time_warping
    module._median_filter = lambda weights, _width: weights
    monkeypatch.setitem(
        sys.modules,
        "transformers.models.whisper.generation_whisper",
        module,
    )


class _FakeWhisperModel:
    def __init__(self) -> None:
        self.config = SimpleNamespace(decoder_layers=1, median_filter_width=1)


class _FakeGenerateOutput:
    def __init__(
        self,
        *,
        cross_attentions: list[list[torch.Tensor]],
        sequences: torch.Tensor,
        beam_indices: torch.Tensor | None = None,
    ) -> None:
        self.cross_attentions = cross_attentions
        self.sequences = sequences
        self.beam_indices = beam_indices

    def __contains__(self, key: str) -> bool:
        return key == "beam_indices" and self.beam_indices is not None


def _attention_step(*batch_rows: list[float]) -> list[torch.Tensor]:
    return [
        torch.tensor(batch_rows, dtype=torch.float32).unsqueeze(1).unsqueeze(2)
    ]


def test_load_audio_supports_dtype_conversion() -> None:
    audio, _ = load_audio(TEST_AUDIO, dtype=torch.float64)
    assert audio.dtype == torch.float64


@pytest.mark.parametrize(
    ("subtype", "expected_bits"),
    [
        ("PCM_16", 16),
        ("PCM_24", 24),
        ("PCM_32", 32),
        ("FLOAT", 32),
        ("PCM_S8", 8),
        ("PCM_U8", 8),
    ],
)
def test_audio_info_detects_bit_depth_from_subtype(
    monkeypatch,
    subtype: str,
    expected_bits: int,
) -> None:
    monkeypatch.setattr(
        "batchalign.inference.audio.sf.info",
        lambda _path: SimpleNamespace(
            samplerate=16000,
            frames=12,
            channels=1,
            subtype=subtype,
        ),
    )

    info = audio_info("fake.wav")

    assert info.bits_per_sample == expected_bits
    assert info.encoding == subtype


def test_chunk_returns_empty_when_end_precedes_begin() -> None:
    audio = ASRAudioFile("fake.wav", torch.arange(50, dtype=torch.float32), 1000)
    assert audio.chunk(10, 10).numel() == 0


def test_eager_chunk_returns_tensor_slice() -> None:
    audio = ASRAudioFile("fake.wav", torch.arange(50, dtype=torch.float32), 1000)
    assert torch.equal(audio.chunk(10, 20), audio.tensor[10:20])


def test_lazy_chunk_caches_and_evicts_oldest_entry() -> None:
    calls: list[tuple[int, int]] = []

    def fake_load(
        _path: str,
        frame_offset: int = 0,
        num_frames: int = -1,
        **_kwargs: object,
    ) -> tuple[torch.Tensor, int]:
        calls.append((frame_offset, num_frames))
        return torch.arange(num_frames, dtype=torch.float32).unsqueeze(0), 1000

    audio = ASRAudioFile.lazy("fake.wav", 1000, cache_limit=1, load_audio_fn=fake_load)

    first = audio.chunk(0, 10)
    second = audio.chunk(0, 10)
    audio.chunk(10, 20)
    audio.chunk(0, 10)

    assert first is second
    assert calls == [(0, 10), (10, 10), (0, 10)]


def test_lazy_chunk_falls_back_to_full_load_after_partial_read_error() -> None:
    calls: list[tuple[int, int]] = []

    def fake_load(
        _path: str,
        frame_offset: int = 0,
        num_frames: int = -1,
        **_kwargs: object,
    ) -> tuple[torch.Tensor, int]:
        calls.append((frame_offset, num_frames))
        if num_frames >= 0:
            raise RuntimeError("partial read failed")
        return torch.arange(200, dtype=torch.float32).unsqueeze(0), 1000

    audio = ASRAudioFile.lazy("fake.wav", 1000, load_audio_fn=fake_load)

    chunk = audio.chunk(10, 20)

    assert torch.equal(chunk, torch.arange(10, 20, dtype=torch.float32))
    assert audio._lazy is False
    assert calls == [(10, 10), (0, -1)]


def test_all_loads_full_waveform_and_resamples(monkeypatch) -> None:
    calls: list[tuple[int, int]] = []
    resample_calls: list[tuple[int, int]] = []
    resampled = torch.tensor(
        [[1.0, 3.0, 5.0, 7.0], [2.0, 4.0, 6.0, 8.0]],
        dtype=torch.float32,
    )
    _install_fake_resample(monkeypatch, result=resampled, calls=resample_calls)

    def fake_load(
        _path: str,
        frame_offset: int = 0,
        num_frames: int = -1,
        **_kwargs: object,
    ) -> tuple[torch.Tensor, int]:
        calls.append((frame_offset, num_frames))
        return torch.tensor([[0.0, 1.0], [2.0, 3.0]], dtype=torch.float32), 8000

    audio = ASRAudioFile.lazy("fake.wav", 16000, load_audio_fn=fake_load)

    waveform = audio.all()

    assert calls == [(0, -1)]
    assert resample_calls == [(8000, 16000)]
    assert torch.equal(
        waveform,
        torch.tensor([1.5, 3.5, 5.5, 7.5], dtype=torch.float32),
    )


def test_all_returns_empty_tensor_when_lazy_load_fails() -> None:
    def fake_load(
        _path: str,
        frame_offset: int = 0,
        num_frames: int = -1,
        **_kwargs: object,
    ) -> tuple[torch.Tensor, int]:
        raise RuntimeError("boom")

    audio = ASRAudioFile.lazy("fake.wav", 16000, load_audio_fn=fake_load)

    assert audio.all().numel() == 0


def test_file_identity_is_stable_for_same_file(tmp_path: Path) -> None:
    path = tmp_path / "audio.bin"
    path.write_bytes(b"abcdef")
    audio = ASRAudioFile(str(path), torch.arange(10, dtype=torch.float32), 1000)

    first = audio.file_identity()
    second = audio.file_identity()

    assert first == second
    assert len(first) == 64


def test_hash_helpers_cover_short_and_long_audio(tmp_path: Path) -> None:
    path = tmp_path / "audio.bin"
    path.write_bytes(b"abcdef")
    short = ASRAudioFile(str(path), torch.arange(50, dtype=torch.float32), 1000)
    long = ASRAudioFile(str(path), torch.arange(200, dtype=torch.float32), 1000)

    assert len(short.hash_chunk(0, 50)) == 64
    assert len(long.hash_chunk(0, 200)) == 64
    assert len(short.hash_all()) == 64
    assert len(long.hash_all()) == 64


def test_load_audio_file_returns_lazy_handle_when_rate_matches(monkeypatch) -> None:
    lazy_handle = ASRAudioFile("fake.wav", torch.empty(0), 16000, _lazy=True)

    monkeypatch.setattr(
        "batchalign.inference.audio.audio_info",
        lambda _path: SimpleNamespace(sample_rate=16000),
    )
    monkeypatch.setattr(
        "batchalign.inference.audio.ASRAudioFile.lazy",
        lambda _path, _rate, cache_limit=16, load_audio_fn=None: lazy_handle,
    )
    monkeypatch.setattr(
        "batchalign.inference.audio.load_audio",
        lambda *_args, **_kwargs: (_ for _ in ()).throw(AssertionError("load_audio should not run")),
    )

    assert load_audio_file("fake.wav", target_sample_rate=16000) is lazy_handle


def test_load_audio_file_falls_back_to_eager_on_lazy_failure(monkeypatch) -> None:
    resample_calls: list[tuple[int, int]] = []
    seen_inputs: list[torch.Tensor] = []
    resampled = torch.tensor([9.0, 8.0, 7.0], dtype=torch.float32)
    _install_fake_resample(
        monkeypatch,
        result=resampled,
        calls=resample_calls,
        seen_inputs=seen_inputs,
    )

    monkeypatch.setattr(
        "batchalign.inference.audio.audio_info",
        lambda _path: (_ for _ in ()).throw(RuntimeError("no metadata")),
    )
    monkeypatch.setattr(
        "batchalign.inference.audio.load_audio",
        lambda _path: (
            torch.tensor([[1.0, 3.0, 5.0], [2.0, 4.0, 6.0]], dtype=torch.float32),
            8000,
        ),
    )

    handle = load_audio_file("broken.mp3", target_sample_rate=16000)

    assert handle.file == "broken.mp3"
    assert handle.rate == 16000
    assert torch.equal(handle.tensor, resampled)
    assert torch.equal(
        seen_inputs[0],
        torch.tensor([1.5, 3.5, 5.5], dtype=torch.float32),
    )
    assert resample_calls == [(8000, 16000)]


def test_load_audio_file_resamples_when_lazy_sample_rate_mismatches(monkeypatch) -> None:
    resample_calls: list[tuple[int, int]] = []
    seen_inputs: list[torch.Tensor] = []
    resampled = torch.tensor([4.0, 5.0], dtype=torch.float32)
    _install_fake_resample(
        monkeypatch,
        result=resampled,
        calls=resample_calls,
        seen_inputs=seen_inputs,
    )
    lazy_handle = ASRAudioFile("slow.wav", torch.empty(0), 8000, _lazy=True)

    monkeypatch.setattr(
        "batchalign.inference.audio.audio_info",
        lambda _path: SimpleNamespace(sample_rate=8000),
    )
    monkeypatch.setattr(
        "batchalign.inference.audio.ASRAudioFile.lazy",
        lambda _path, _rate, cache_limit=16, load_audio_fn=None: lazy_handle,
    )
    monkeypatch.setattr(
        "batchalign.inference.audio.load_audio",
        lambda _path: (
            torch.tensor([[10.0, 12.0], [14.0, 16.0]], dtype=torch.float32),
            8000,
        ),
    )

    handle = load_audio_file("slow.wav", target_sample_rate=16000)

    assert handle is not lazy_handle
    assert handle.rate == 16000
    assert torch.equal(handle.tensor, resampled)
    assert torch.equal(
        seen_inputs[0],
        torch.tensor([12.0, 14.0], dtype=torch.float32),
    )
    assert resample_calls == [(8000, 16000)]


def test_extract_token_timestamps_handles_integer_num_frames(monkeypatch) -> None:
    recorded: list[np.ndarray] = []
    _install_whisper_generation_helpers(monkeypatch, recorded_matrices=recorded)
    output = _FakeGenerateOutput(
        cross_attentions=[
            _attention_step([1.0, 2.0, 3.0, 4.0]),
            _attention_step([2.0, 3.0, 4.0, 5.0]),
        ],
        sequences=torch.tensor([[1, 2]], dtype=torch.int64),
    )

    timestamps = _extract_token_timestamps(
        _FakeWhisperModel(),
        output,
        [(0, 0)],
        num_frames=4,
    )

    assert timestamps.shape == (1, 3)
    assert recorded[0].shape == (2, 2)


def test_extract_token_timestamps_handles_uniform_list_num_frames(monkeypatch) -> None:
    recorded: list[np.ndarray] = []
    _install_whisper_generation_helpers(monkeypatch, recorded_matrices=recorded)
    output = _FakeGenerateOutput(
        cross_attentions=[
            _attention_step([1.0, 2.0, 3.0, 4.0]),
            _attention_step([2.0, 3.0, 4.0, 5.0]),
        ],
        sequences=torch.tensor([[1, 2]], dtype=torch.int64),
    )

    timestamps = _extract_token_timestamps(
        _FakeWhisperModel(),
        output,
        [(0, 0)],
        num_frames=[4, 4],
    )

    assert timestamps.shape == (1, 3)
    assert recorded[0].shape == (2, 2)


def test_extract_token_timestamps_handles_uniform_tensor_num_frames(monkeypatch) -> None:
    recorded: list[np.ndarray] = []
    _install_whisper_generation_helpers(monkeypatch, recorded_matrices=recorded)
    output = _FakeGenerateOutput(
        cross_attentions=[
            _attention_step([1.0, 2.0, 3.0, 4.0]),
            _attention_step([2.0, 3.0, 4.0, 5.0]),
        ],
        sequences=torch.tensor([[1, 2]], dtype=torch.int64),
    )

    timestamps = _extract_token_timestamps(
        _FakeWhisperModel(),
        output,
        [(0, 0)],
        num_frames=torch.tensor([4, 4]),
    )

    assert timestamps.shape == (1, 3)
    assert recorded[0].shape == (2, 2)


def test_extract_token_timestamps_handles_per_batch_num_frames(monkeypatch) -> None:
    recorded: list[np.ndarray] = []
    _install_whisper_generation_helpers(monkeypatch, recorded_matrices=recorded)
    output = _FakeGenerateOutput(
        cross_attentions=[
            _attention_step(
                [1.0, 2.0, 3.0, 4.0],
                [1.5, 2.5, 3.5, 4.5],
            ),
            _attention_step(
                [2.0, 3.0, 4.0, 5.0],
                [2.5, 3.5, 4.5, 5.5],
            ),
        ],
        sequences=torch.tensor([[1, 2], [1, 2]], dtype=torch.int64),
    )

    timestamps = _extract_token_timestamps(
        _FakeWhisperModel(),
        output,
        [(0, 0)],
        num_frames=[4, 6],
    )

    assert timestamps.shape == (2, 3)
    assert [matrix.shape for matrix in recorded] == [(2, 2), (2, 3)]


def test_extract_token_timestamps_handles_beam_indices(monkeypatch) -> None:
    recorded: list[np.ndarray] = []
    _install_whisper_generation_helpers(monkeypatch, recorded_matrices=recorded)
    output = _FakeGenerateOutput(
        cross_attentions=[
            _attention_step(
                [1.0, 2.0, 3.0, 4.0],
                [1.1, 2.1, 3.1, 4.1],
            ),
            _attention_step(
                [2.0, 3.0, 4.0, 5.0],
                [2.1, 3.1, 4.1, 5.1],
            ),
        ],
        sequences=torch.tensor([[1, 2]], dtype=torch.int64),
        beam_indices=torch.tensor([[0, 1, -1]], dtype=torch.int64),
    )

    timestamps = _extract_token_timestamps(
        _FakeWhisperModel(),
        output,
        [(0, 0)],
        num_frames=4,
        num_input_ids=0,
    )

    assert timestamps.shape == (1, 3)
    assert recorded[0].shape == (2, 2)
