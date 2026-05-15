"""Focused tests for the live worker-protocol V2 speaker executor."""

from __future__ import annotations

from pathlib import Path

import numpy as np
import pytest
from pydantic import ValidationError

from batchalign.device import DevicePolicy
from batchalign.inference.speaker import SpeakerResponse, SpeakerSegment
from batchalign.worker._speaker_v2 import (
    SpeakerExecutionHostV2,
    build_default_speaker_execution_host_v2,
    execute_speaker_request_v2,
)
from batchalign.worker._types_v2 import (
    ExecuteErrorV2,
    ExecuteRequestV2,
    ExecuteSuccessV2,
    InferenceTaskV2,
    PreparedAudioEncodingV2,
    PreparedAudioRefV2,
    ProtocolErrorCodeV2,
    SpeakerBackendV2,
    SpeakerPreparedAudioInputV2,
    SpeakerRequestV2,
    SpeakerResultV2,
)


def _write_pcm_f32le(path: Path, samples: np.ndarray) -> None:
    """Write little-endian float32 PCM speaker fixture data to disk."""

    path.write_bytes(samples.astype("<f4").tobytes())


def _make_request(
    tmp_path: Path,
    *,
    backend: SpeakerBackendV2 = SpeakerBackendV2.PYANNOTE,
    expected_speakers: int | None = 2,
    channels: int = 1,
    byte_len: int | None = None,
) -> ExecuteRequestV2:
    """Create one live V2 speaker request with prepared audio."""

    frame_count = 4
    sample_count = frame_count * channels
    audio_path = tmp_path / "speaker-audio.pcm"
    _write_pcm_f32le(
        audio_path,
        np.asarray([float(i) / 10.0 for i in range(1, sample_count + 1)], dtype=np.float32),
    )
    if byte_len is None:
        byte_len = sample_count * 4

    return ExecuteRequestV2(
        request_id="req-speaker-v2-1",
        task=InferenceTaskV2.SPEAKER,
        payload=SpeakerRequestV2(
            backend=backend,
            input=SpeakerPreparedAudioInputV2(audio_ref_id="audio-ref-speaker-1"),
            expected_speakers=expected_speakers,
        ),
        attachments=[
            PreparedAudioRefV2(
                id="audio-ref-speaker-1",
                path=str(audio_path),
                encoding=PreparedAudioEncodingV2.PCM_F32LE,
                channels=channels,
                sample_rate_hz=16000,
                frame_count=frame_count,
                byte_offset=0,
                byte_len=byte_len,
            )
        ],
    )


def _assert_error_response(
    response,
    code: ProtocolErrorCodeV2,
    message_fragment: str | None = None,
) -> None:
    """Assert one typed speaker error response."""

    assert isinstance(response.outcome, ExecuteErrorV2)
    assert response.outcome.code is code
    if message_fragment is not None:
        assert message_fragment in response.outcome.message
    assert response.result is None


def test_execute_speaker_request_v2_returns_typed_segments(tmp_path: Path) -> None:
    """Speaker V2 execution should return the typed segment payload."""

    response = execute_speaker_request_v2(
        _make_request(tmp_path),
        SpeakerExecutionHostV2(
            pyannote_prepared_audio_runner=lambda audio, sample_rate_hz, num_speakers: SpeakerResponse(
                segments=[
                    SpeakerSegment(
                        start_ms=10,
                        end_ms=25,
                        speaker=f"SPEAKER_{num_speakers}_{sample_rate_hz}_{audio.shape[0]}",
                    )
                ]
            )
        ),
    )

    assert isinstance(response.outcome, ExecuteSuccessV2)
    assert isinstance(response.result, SpeakerResultV2)
    assert response.result.segments[0].speaker == "SPEAKER_2_16000_4"
    assert response.result.segments[0].start_ms == 10


def test_execute_speaker_request_v2_defaults_expected_speakers_to_two(tmp_path: Path) -> None:
    """Missing expected_speakers should preserve the batchalign default of 2."""

    captured: dict[str, object] = {}

    def runner(audio, sample_rate_hz, num_speakers):
        captured["shape"] = audio.shape
        captured["sample_rate_hz"] = sample_rate_hz
        captured["num_speakers"] = num_speakers
        return SpeakerResponse(segments=[])

    response = execute_speaker_request_v2(
        _make_request(tmp_path, expected_speakers=None),
        SpeakerExecutionHostV2(pyannote_prepared_audio_runner=runner),
    )

    assert isinstance(response.outcome, ExecuteSuccessV2)
    assert captured == {"shape": (4,), "sample_rate_hz": 16000, "num_speakers": 2}


def test_execute_speaker_request_v2_routes_nemo_backend(tmp_path: Path) -> None:
    """NeMo requests should use the injected NeMo host, not the Pyannote host."""

    captured: dict[str, object] = {}

    def runner(audio, sample_rate_hz, num_speakers):
        captured["shape"] = audio.shape
        captured["sample_rate_hz"] = sample_rate_hz
        captured["num_speakers"] = num_speakers
        return SpeakerResponse(
            segments=[
                SpeakerSegment(start_ms=5, end_ms=15, speaker=f"SPEAKER_{num_speakers}")
            ]
        )

    response = execute_speaker_request_v2(
        _make_request(tmp_path, backend=SpeakerBackendV2.NEMO, expected_speakers=3),
        SpeakerExecutionHostV2(nemo_prepared_audio_runner=runner),
    )

    assert isinstance(response.outcome, ExecuteSuccessV2)
    assert isinstance(response.result, SpeakerResultV2)
    assert response.result.segments[0].speaker == "SPEAKER_3"
    assert captured == {"shape": (4,), "sample_rate_hz": 16000, "num_speakers": 3}


def test_execute_speaker_request_v2_rejects_wrong_task() -> None:
    """Speaker executor should fail with a typed protocol error on wrong tasks."""

    response = execute_speaker_request_v2(
        ExecuteRequestV2(
            request_id="req-speaker-v2-wrong-task",
            task=InferenceTaskV2.ASR,
            payload=SpeakerRequestV2(
                backend=SpeakerBackendV2.PYANNOTE,
                input=SpeakerPreparedAudioInputV2(audio_ref_id="audio-ref-speaker-2"),
            ),
            attachments=[],
        ),
        SpeakerExecutionHostV2(),
    )

    _assert_error_response(response, ProtocolErrorCodeV2.INVALID_PAYLOAD, "expected speaker task")


def test_speaker_provider_media_requests_fail_schema_validation() -> None:
    """Legacy speaker provider-media requests should be rejected at the schema boundary."""

    with pytest.raises(ValidationError):
        ExecuteRequestV2.model_validate(
            {
                "request_id": "req-speaker-v2-legacy",
                "task": "speaker",
                "payload": {
                    "kind": "speaker",
                    "backend": "pyannote",
                    "input": {
                        "kind": "provider_media",
                        "media_path": "/tmp/meeting.wav",
                    },
                },
                "attachments": [],
            }
        )


def test_default_speaker_host_forwards_device_policy(monkeypatch) -> None:
    """The live speaker host should forward the injected device policy."""

    captured: dict[str, object] = {}

    def fake_infer(audio, sample_rate_hz, *, num_speakers, engine, device_policy):
        captured["shape"] = audio.shape
        captured["sample_rate_hz"] = sample_rate_hz
        captured["num_speakers"] = num_speakers
        captured["engine"] = engine
        captured["device_policy"] = device_policy
        return SpeakerResponse(segments=[])

    monkeypatch.setattr(
        "batchalign.worker._speaker_v2.infer_speaker_prepared_audio",
        fake_infer,
    )

    host = build_default_speaker_execution_host_v2(DevicePolicy(force_cpu=True))
    assert host.nemo_prepared_audio_runner is not None
    host.nemo_prepared_audio_runner(np.asarray([0.1, 0.2], dtype=np.float32), 16000, 3)

    assert captured["shape"] == (2,)
    assert captured["sample_rate_hz"] == 16000
    assert captured["num_speakers"] == 3
    assert captured["engine"] == "nemo"
    assert captured["device_policy"] == DevicePolicy(force_cpu=True)


def test_missing_speaker_attachment_returns_typed_error(tmp_path: Path) -> None:
    """Missing prepared audio should become a typed protocol error."""

    request = _make_request(tmp_path)
    request.attachments = []

    response = execute_speaker_request_v2(
        request,
        SpeakerExecutionHostV2(pyannote_prepared_audio_runner=lambda *_args: SpeakerResponse(segments=[])),
    )

    _assert_error_response(response, ProtocolErrorCodeV2.MISSING_ATTACHMENT, "missing worker protocol V2 attachment")


def test_multichannel_speaker_audio_is_rejected(tmp_path: Path) -> None:
    """The live speaker path should reject multi-channel prepared audio explicitly."""

    response = execute_speaker_request_v2(
        _make_request(tmp_path, channels=2, byte_len=32),
        SpeakerExecutionHostV2(pyannote_prepared_audio_runner=lambda *_args: SpeakerResponse(segments=[])),
    )

    _assert_error_response(response, ProtocolErrorCodeV2.INVALID_PAYLOAD, "mono prepared audio")


def test_malformed_speaker_audio_returns_attachment_unreadable(tmp_path: Path) -> None:
    """Truncated prepared audio should map to ATTACHMENT_UNREADABLE."""

    response = execute_speaker_request_v2(
        _make_request(tmp_path, byte_len=15),
        SpeakerExecutionHostV2(pyannote_prepared_audio_runner=lambda *_args: SpeakerResponse(segments=[])),
    )

    _assert_error_response(
        response,
        ProtocolErrorCodeV2.ATTACHMENT_UNREADABLE,
        "has 15 bytes, expected 16",
    )


def test_missing_nemo_host_returns_model_unavailable(tmp_path: Path) -> None:
    """Requests for unloaded speaker hosts should fail explicitly."""

    response = execute_speaker_request_v2(
        _make_request(tmp_path, backend=SpeakerBackendV2.NEMO),
        SpeakerExecutionHostV2(),
    )

    _assert_error_response(response, ProtocolErrorCodeV2.MODEL_UNAVAILABLE, "no NeMo speaker host")


def test_speaker_runtime_failures_are_typed(tmp_path: Path) -> None:
    """Unexpected speaker host failures should surface as runtime failures."""

    def boom(*_args):
        raise RuntimeError("speaker host exploded")

    response = execute_speaker_request_v2(
        _make_request(tmp_path),
        SpeakerExecutionHostV2(pyannote_prepared_audio_runner=boom),
    )

    _assert_error_response(response, ProtocolErrorCodeV2.RUNTIME_FAILURE, "speaker host exploded")


def test_invalid_numeric_attachment_becomes_invalid_payload_even_if_validation_is_bypassed(
    tmp_path: Path,
) -> None:
    """Rust should still reject bad prepared-audio numerics if a caller bypasses Pydantic."""

    request = _make_request(tmp_path)
    raw_attachment = request.attachments[0].model_dump()
    raw_attachment["sample_rate_hz"] = 0
    bad_attachment = PreparedAudioRefV2.model_construct(**raw_attachment)
    bad_request = ExecuteRequestV2.model_construct(
        request_id=request.request_id,
        task=request.task,
        payload=request.payload,
        attachments=[bad_attachment],
    )

    response = execute_speaker_request_v2(
        bad_request,
        SpeakerExecutionHostV2(pyannote_prepared_audio_runner=lambda *_args: SpeakerResponse(segments=[])),
    )

    _assert_error_response(
        response,
        ProtocolErrorCodeV2.INVALID_PAYLOAD,
        "positive sample_rate_hz",
    )


def test_invalid_speaker_host_output_becomes_runtime_failure(tmp_path: Path) -> None:
    """Malformed speaker host output should be classified as runtime failure."""

    response = execute_speaker_request_v2(
        _make_request(tmp_path),
        SpeakerExecutionHostV2(
            pyannote_prepared_audio_runner=lambda *_args: SpeakerResponse(
                segments=[
                    SpeakerSegment(
                        start_ms=1200,
                        end_ms=200,
                        speaker="SPEAKER_BAD",
                    )
                ]
            )
        ),
    )

    _assert_error_response(
        response,
        ProtocolErrorCodeV2.RUNTIME_FAILURE,
        "invalid speaker host output",
    )
