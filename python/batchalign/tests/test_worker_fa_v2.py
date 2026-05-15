"""Tests for the staged worker-protocol V2 forced-alignment executor."""

from __future__ import annotations

import json
import threading
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path

import numpy as np

from batchalign.worker._fa_v2 import (
    ForcedAlignmentExecutionHostV2,
    execute_forced_alignment_request_v2,
)
from batchalign.worker._types_v2 import (
    ExecuteErrorV2,
    ExecuteRequestV2,
    ExecuteSuccessV2,
    FaBackendV2,
    FaTextModeV2,
    ForcedAlignmentRequestV2,
    IndexedWordTimingResultV2,
    InferenceTaskV2,
    PreparedAudioEncodingV2,
    PreparedAudioRefV2,
    PreparedTextEncodingV2,
    PreparedTextRefV2,
    ProtocolErrorCodeV2,
    WhisperTokenTimingResultV2,
)


def _write_pcm_f32le(path: Path, samples: np.ndarray) -> None:
    """Write little-endian float32 PCM test data to disk."""

    path.write_bytes(samples.astype("<f4").tobytes())


def _write_payload(path: Path) -> None:
    """Write a minimal prepared FA payload for staged V2 executor tests."""

    path.write_text(
        json.dumps(
            {
                "words": ["hello", "world"],
                "word_ids": ["u0:w0", "u0:w1"],
                "word_utterance_indices": [0, 0],
                "word_utterance_word_indices": [0, 1],
            }
        ),
        encoding="utf-8",
    )


def _make_request(tmp_path: Path, *, backend: FaBackendV2, text_mode: FaTextModeV2) -> ExecuteRequestV2:
    """Create one staged V2 FA execute request with prepared artifacts."""

    payload_path = tmp_path / "payload.json"
    audio_path = tmp_path / "audio.pcm"
    _write_payload(payload_path)
    _write_pcm_f32le(audio_path, np.asarray([0.1, 0.2, 0.3, 0.4], dtype=np.float32))

    return ExecuteRequestV2(
        request_id="req-fa-stage-1",
        task=InferenceTaskV2.FORCED_ALIGNMENT,
        payload=ForcedAlignmentRequestV2(
            backend=backend,
            payload_ref_id="payload-ref-1",
            audio_ref_id="audio-ref-1",
            text_mode=text_mode,
            pauses=True,
        ),
        attachments=[
            PreparedTextRefV2(
                id="payload-ref-1",
                path=str(payload_path),
                encoding=PreparedTextEncodingV2.UTF8_JSON,
                byte_offset=0,
                byte_len=payload_path.stat().st_size,
            ),
            PreparedAudioRefV2(
                id="audio-ref-1",
                path=str(audio_path),
                encoding=PreparedAudioEncodingV2.PCM_F32LE,
                channels=1,
                sample_rate_hz=16000,
                frame_count=4,
                byte_offset=0,
                byte_len=16,
            ),
        ],
    )


def test_executes_whisper_fa_v2_request(tmp_path: Path) -> None:
    """The staged V2 executor should return raw Whisper token timing output."""

    captured: dict[str, object] = {}

    def whisper_runner(audio: np.ndarray, text: str, pauses: bool) -> list[tuple[str, float]]:
        captured["shape"] = audio.shape
        captured["text"] = text
        captured["pauses"] = pauses
        return [("hello", 0.12), ("world", 0.38)]

    response = execute_forced_alignment_request_v2(
        _make_request(tmp_path, backend=FaBackendV2.WHISPER, text_mode=FaTextModeV2.SPACE_JOINED),
        ForcedAlignmentExecutionHostV2(whisper_runner=whisper_runner),
    )

    assert isinstance(response.outcome, ExecuteSuccessV2)
    assert isinstance(response.result, WhisperTokenTimingResultV2)
    assert response.result.tokens[0].text == "hello"
    assert response.result.tokens[1].time_s == 0.38
    assert captured == {"shape": (4,), "text": "hello world", "pauses": True}


def test_executes_wave2vec_fa_v2_request(tmp_path: Path) -> None:
    """The staged V2 executor should project indexed word timings for wave2vec."""

    def wave2vec_runner(audio: np.ndarray, words: list[str]) -> list[tuple[str, tuple[int, int]]]:
        assert audio.shape == (4,)
        assert words == ["hello", "world"]
        return [("hello", (10, 40)), ("world", (40, 90))]

    response = execute_forced_alignment_request_v2(
        _make_request(tmp_path, backend=FaBackendV2.WAVE2VEC, text_mode=FaTextModeV2.SPACE_JOINED),
        ForcedAlignmentExecutionHostV2(wave2vec_runner=wave2vec_runner),
    )

    assert isinstance(response.outcome, ExecuteSuccessV2)
    assert isinstance(response.result, IndexedWordTimingResultV2)
    assert response.result.indexed_timings[0].start_ms == 10
    assert response.result.indexed_timings[1].end_ms == 90


def test_executes_cantonese_wave2vec_fa_v2_request(tmp_path: Path) -> None:
    """The FA executor should preserve the internal Cantonese callback contract."""

    captured: dict[str, object] = {}

    def canto_runner(audio, payload, request):
        captured["shape"] = audio.shape
        captured["words"] = payload.words
        captured["text_mode"] = request.text_mode
        return [(payload.words[0], (50, 120)), (payload.words[1], (130, 220))]

    response = execute_forced_alignment_request_v2(
        _make_request(tmp_path, backend=FaBackendV2.WAV2VEC_CANTO, text_mode=FaTextModeV2.CHAR_JOINED),
        ForcedAlignmentExecutionHostV2(canto_runner=canto_runner),
    )

    assert isinstance(response.outcome, ExecuteSuccessV2)
    assert isinstance(response.result, IndexedWordTimingResultV2)
    assert response.result.indexed_timings[0].start_ms == 50
    assert response.result.indexed_timings[1].end_ms == 220
    assert captured == {
        "shape": (4,),
        "words": ["hello", "world"],
        "text_mode": FaTextModeV2.CHAR_JOINED,
    }


def test_returns_missing_attachment_error_for_invalid_request(tmp_path: Path) -> None:
    """Missing prepared artifacts should become typed protocol errors."""

    request = _make_request(tmp_path, backend=FaBackendV2.WHISPER, text_mode=FaTextModeV2.SPACE_JOINED)
    request.attachments = request.attachments[:1]

    response = execute_forced_alignment_request_v2(
        request,
        ForcedAlignmentExecutionHostV2(whisper_runner=lambda *_args: []),
    )

    assert isinstance(response.outcome, ExecuteErrorV2)
    assert response.outcome.code is ProtocolErrorCodeV2.MISSING_ATTACHMENT
    assert response.result is None


def test_invalid_numeric_attachment_becomes_invalid_payload_even_if_validation_is_bypassed(
    tmp_path: Path,
) -> None:
    """Rust should reject bad FA prepared-audio numerics even when Python is bypassed."""

    request = _make_request(tmp_path, backend=FaBackendV2.WHISPER, text_mode=FaTextModeV2.SPACE_JOINED)
    raw_attachment = request.attachments[1].model_dump()
    raw_attachment["sample_rate_hz"] = 0
    bad_attachment = PreparedAudioRefV2.model_construct(**raw_attachment)
    bad_request = ExecuteRequestV2.model_construct(
        request_id=request.request_id,
        task=request.task,
        payload=request.payload,
        attachments=[request.attachments[0], bad_attachment],
    )

    response = execute_forced_alignment_request_v2(
        bad_request,
        ForcedAlignmentExecutionHostV2(whisper_runner=lambda *_args: []),
    )

    assert isinstance(response.outcome, ExecuteErrorV2)
    assert response.outcome.code is ProtocolErrorCodeV2.INVALID_PAYLOAD
    assert "positive sample_rate_hz" in response.outcome.message
    assert response.result is None


def test_returns_model_unavailable_for_unwired_cantonese_backend(tmp_path: Path) -> None:
    """Backends without an installed host should fail explicitly."""

    response = execute_forced_alignment_request_v2(
        _make_request(
            tmp_path,
            backend=FaBackendV2.WAV2VEC_CANTO,
            text_mode=FaTextModeV2.CHAR_JOINED,
        ),
        ForcedAlignmentExecutionHostV2(),
    )

    assert isinstance(response.outcome, ExecuteErrorV2)
    assert response.outcome.code is ProtocolErrorCodeV2.MODEL_UNAVAILABLE
    assert "Cantonese FA host" in response.outcome.message


def test_invalid_whisper_fa_host_output_becomes_runtime_failure(tmp_path: Path) -> None:
    """Malformed Whisper FA host output should be classified as runtime failure."""

    response = execute_forced_alignment_request_v2(
        _make_request(tmp_path, backend=FaBackendV2.WHISPER, text_mode=FaTextModeV2.SPACE_JOINED),
        ForcedAlignmentExecutionHostV2(
            whisper_runner=lambda _audio, _text, _pauses: [("hello", float("nan"))]
        ),
    )

    assert isinstance(response.outcome, ExecuteErrorV2)
    assert response.outcome.code is ProtocolErrorCodeV2.RUNTIME_FAILURE
    assert "invalid forced-alignment host output" in response.outcome.message
    assert response.result is None


def test_invalid_wave2vec_fa_host_output_becomes_runtime_failure(tmp_path: Path) -> None:
    """Malformed wave2vec FA host output should be classified as runtime failure."""

    response = execute_forced_alignment_request_v2(
        _make_request(tmp_path, backend=FaBackendV2.WAVE2VEC, text_mode=FaTextModeV2.SPACE_JOINED),
        ForcedAlignmentExecutionHostV2(
            wave2vec_runner=lambda _audio, _words: [("hello", (40, 10))]
        ),
    )

    assert isinstance(response.outcome, ExecuteErrorV2)
    assert response.outcome.code is ProtocolErrorCodeV2.RUNTIME_FAILURE
    assert "invalid forced-alignment host output" in response.outcome.message
    assert response.result is None


def _make_request_in_dir(thread_dir: Path, thread_index: int) -> ExecuteRequestV2:
    """Create a V2 FA wave2vec request with files in a per-thread directory.

    Each concurrent thread needs its own artifact files to avoid file-path
    collisions when multiple requests run simultaneously in a thread pool.
    """
    thread_dir.mkdir(exist_ok=True)
    payload_path = thread_dir / "payload.json"
    audio_path = thread_dir / "audio.pcm"
    _write_payload(payload_path)
    # Use 160 samples (10 ms of audio at 16 kHz) — enough to pass frame validation
    _write_pcm_f32le(audio_path, np.ones(160, dtype=np.float32) * 0.1)
    return ExecuteRequestV2(
        request_id=f"req-concurrent-{thread_index}",
        task=InferenceTaskV2.FORCED_ALIGNMENT,
        payload=ForcedAlignmentRequestV2(
            backend=FaBackendV2.WAVE2VEC,
            payload_ref_id=f"payload-{thread_index}",
            audio_ref_id=f"audio-{thread_index}",
            text_mode=FaTextModeV2.SPACE_JOINED,
            pauses=True,
        ),
        attachments=[
            PreparedTextRefV2(
                id=f"payload-{thread_index}",
                path=str(payload_path),
                encoding=PreparedTextEncodingV2.UTF8_JSON,
                byte_offset=0,
                byte_len=payload_path.stat().st_size,
            ),
            PreparedAudioRefV2(
                id=f"audio-{thread_index}",
                path=str(audio_path),
                encoding=PreparedAudioEncodingV2.PCM_F32LE,
                channels=1,
                sample_rate_hz=16000,
                frame_count=160,
                byte_offset=0,
                byte_len=160 * 4,
            ),
        ],
    )


def test_wave2vec_fa_requests_are_serialized_under_thread_pool(tmp_path: Path) -> None:
    """Concurrent wave2vec FA requests from a thread pool must not overlap.

    The torchaudio MMS_FA / forced_align kernel is not safe for concurrent CPU
    execution from multiple threads. When the Python GPU worker uses
    ``_serve_stdio_concurrent(max_threads=4)``, up to 4 requests can reach the
    wave2vec runner simultaneously — causing SIGSEGV/SIGABRT in LibTorch that
    kills the worker process and fails ALL pending file alignments in the job.

    Regression test for the 2026-04-09 production crash on net where 78/304
    CA corpus files failed alignment in job bd69b68b.

    RED (before fix): ``max_concurrent_calls > 1`` — multiple threads entered
    the runner at the same time (confirmed by counter exceeding 1 while another
    thread was sleeping inside the runner).

    GREEN (after fix): ``max_concurrent_calls == 1`` — the module-level
    ``_fa_inference_lock`` in ``execute_forced_alignment_request_v2`` serializes
    all FA backend calls so only one runner runs at any given moment.
    """
    active = 0
    max_concurrent = 0
    tracking_lock = threading.Lock()

    def serialization_checking_runner(
        audio: np.ndarray,
        words: list[str],
    ) -> list[tuple[str, tuple[int, int]]]:
        """Track concurrent entry count; sleep so overlap is detectable."""
        nonlocal active, max_concurrent
        with tracking_lock:
            active += 1
            if active > max_concurrent:
                max_concurrent = active
        # Hold for 50 ms so threads started nearly simultaneously will overlap
        # if there is no serialization lock on the outer boundary.
        time.sleep(0.05)
        with tracking_lock:
            active -= 1
        return [(w, (i * 100, (i + 1) * 100)) for i, w in enumerate(words)]

    host = ForcedAlignmentExecutionHostV2(wave2vec_runner=serialization_checking_runner)
    # Match the default gpu_thread_pool_size so the test reflects real concurrency
    n_threads = 4
    requests = [
        _make_request_in_dir(tmp_path / f"thread_{i}", i) for i in range(n_threads)
    ]

    with ThreadPoolExecutor(max_workers=n_threads) as pool:
        futures = [
            pool.submit(execute_forced_alignment_request_v2, req, host)
            for req in requests
        ]
        responses = [f.result() for f in as_completed(futures)]

    # Every request must succeed (the fake runner always returns valid output)
    for response in responses:
        assert isinstance(response.outcome, ExecuteSuccessV2), (
            f"Expected success but got: {response.outcome}"
        )

    # At most one wave2vec call may be active at a time.  The torchaudio
    # forced_align kernel is not thread-safe for concurrent CPU execution; a
    # module-level lock in execute_forced_alignment_request_v2 is the fix.
    assert max_concurrent == 1, (
        f"wave2vec runner was entered by {max_concurrent} threads simultaneously "
        f"(expected 1). torchaudio forced_align is not thread-safe under concurrent "
        f"CPU load and will SIGSEGV/SIGABRT the worker process."
    )
