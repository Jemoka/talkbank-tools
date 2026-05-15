"""Tests for the live worker-protocol V2 ASR executor."""

from __future__ import annotations

from pathlib import Path

import numpy as np

from batchalign.worker._asr_v2 import AsrExecutionHostV2, execute_asr_request_v2
from batchalign.inference.asr import AsrElement, AsrMonologue, MonologueAsrResponse
from batchalign.worker._types_v2 import (
    AsrBackendV2,
    AsrElementKindV2,
    AsrRequestV2,
    ExecuteErrorV2,
    ExecuteRequestV2,
    ExecuteSuccessV2,
    InferenceTaskV2,
    MonologueAsrResultV2,
    PreparedAudioEncodingV2,
    PreparedAudioInputV2,
    PreparedAudioRefV2,
    ProviderMediaInputV2,
    ProtocolErrorCodeV2,
    WhisperChunkResultPayloadV2,
    WhisperChunkResultV2,
    WhisperChunkSpanV2,
)


def _write_pcm_f32le(path: Path, samples: np.ndarray) -> None:
    """Write little-endian float32 PCM test data to disk."""

    path.write_bytes(samples.astype("<f4").tobytes())


def _make_request(
    tmp_path: Path,
    *,
    backend: AsrBackendV2 = AsrBackendV2.LOCAL_WHISPER,
) -> ExecuteRequestV2:
    """Create one live V2 ASR execute request with prepared audio."""

    audio_path = tmp_path / "audio.pcm"
    _write_pcm_f32le(audio_path, np.asarray([0.1, 0.2, 0.3, 0.4], dtype=np.float32))

    return ExecuteRequestV2(
        request_id="req-asr-v2-1",
        task=InferenceTaskV2.ASR,
        payload=AsrRequestV2(
            lang="eng",
            backend=backend,
            input=PreparedAudioInputV2(audio_ref_id="audio-ref-1"),
        ),
        attachments=[
            PreparedAudioRefV2(
                id="audio-ref-1",
                path=str(audio_path),
                encoding=PreparedAudioEncodingV2.PCM_F32LE,
                channels=1,
                sample_rate_hz=16000,
                frame_count=4,
                byte_offset=0,
                byte_len=16,
            )
        ],
    )


def _make_provider_request(
    *,
    backend: AsrBackendV2 = AsrBackendV2.HK_TENCENT,
) -> ExecuteRequestV2:
    """Create one live V2 ASR request that uses provider-media input."""

    return ExecuteRequestV2(
        request_id="req-asr-v2-provider-1",
        task=InferenceTaskV2.ASR,
        payload=AsrRequestV2(
            lang="yue",
            backend=backend,
            input=ProviderMediaInputV2(
                media_path="/tmp/provider.wav",
                num_speakers=2,
            ),
        ),
        attachments=[],
    )


def test_executes_local_whisper_asr_v2_request(tmp_path: Path) -> None:
    """The live V2 executor should return typed Whisper chunk output."""

    captured: dict[str, object] = {}

    def runner(audio: np.ndarray, lang: str) -> WhisperChunkResultPayloadV2:
        captured["shape"] = audio.shape
        captured["lang"] = lang
        return WhisperChunkResultPayloadV2(
            lang=lang,
            text="hello world",
            chunks=[
                WhisperChunkSpanV2(text="hello", start_s=0.0, end_s=0.5),
                WhisperChunkSpanV2(text="world", start_s=0.5, end_s=1.0),
            ],
        )

    response = execute_asr_request_v2(
        _make_request(tmp_path),
        AsrExecutionHostV2(local_whisper_runner=runner),
    )

    assert isinstance(response.outcome, ExecuteSuccessV2)
    assert isinstance(response.result, WhisperChunkResultV2)
    assert response.result.text == "hello world"
    assert response.result.chunks[1].end_s == 1.0
    assert captured == {"shape": (4,), "lang": "eng"}


def test_returns_missing_attachment_for_invalid_asr_request(tmp_path: Path) -> None:
    """Missing prepared audio should become a typed protocol error."""

    request = _make_request(tmp_path)
    request.attachments = []

    response = execute_asr_request_v2(
        request,
        AsrExecutionHostV2(local_whisper_runner=lambda *_args: None),  # type: ignore[arg-type]
    )

    assert isinstance(response.outcome, ExecuteErrorV2)
    assert response.outcome.code is ProtocolErrorCodeV2.MISSING_ATTACHMENT
    assert response.result is None


def test_invalid_numeric_attachment_becomes_invalid_payload_even_if_validation_is_bypassed(
    tmp_path: Path,
) -> None:
    """Rust should reject bad prepared-audio numerics even when Python is bypassed."""

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

    response = execute_asr_request_v2(
        bad_request,
        AsrExecutionHostV2(
            local_whisper_runner=lambda *_args: WhisperChunkResultPayloadV2(
                lang="eng",
                text="unused",
                chunks=[],
            )
        ),
    )

    assert isinstance(response.outcome, ExecuteErrorV2)
    assert response.outcome.code is ProtocolErrorCodeV2.INVALID_PAYLOAD
    assert "positive sample_rate_hz" in response.outcome.message
    assert response.result is None


def test_returns_model_unavailable_for_unwired_asr_backend(tmp_path: Path) -> None:
    """Cloud/provider ASR backends should fail explicitly on the live V2 path."""

    response = execute_asr_request_v2(
        _make_provider_request(backend=AsrBackendV2.REVAI),
        AsrExecutionHostV2(),
    )

    assert isinstance(response.outcome, ExecuteErrorV2)
    assert response.outcome.code is ProtocolErrorCodeV2.MODEL_UNAVAILABLE
    assert "Rust control plane" in response.outcome.message


def test_executes_provider_media_asr_v2_request() -> None:
    """Provider-media ASR requests should return typed monologue output."""

    captured: dict[str, object] = {}

    def runner(item) -> MonologueAsrResponse:
        captured["audio_path"] = item.audio_path
        captured["lang"] = item.lang
        captured["num_speakers"] = item.num_speakers
        return MonologueAsrResponse(
            lang=item.lang,
            monologues=[
                AsrMonologue(
                    speaker=1,
                    elements=[
                        AsrElement(
                            value="nei5",
                            ts=0.1,
                            end_ts=0.4,
                            type="text",
                            confidence=0.9,
                        ),
                        AsrElement(value="。", type="punctuation"),
                    ],
                )
            ],
        )

    response = execute_asr_request_v2(
        _make_provider_request(),
        AsrExecutionHostV2(hk_tencent_runner=runner),
    )

    assert isinstance(response.outcome, ExecuteSuccessV2)
    assert isinstance(response.result, MonologueAsrResultV2)
    assert response.result.monologues[0].speaker == "1"
    assert response.result.monologues[0].elements[0].kind is AsrElementKindV2.TEXT
    assert (
        response.result.monologues[0].elements[1].kind
        is AsrElementKindV2.PUNCTUATION
    )
    assert captured == {
        "audio_path": "/tmp/provider.wav",
        "lang": "yue",
        "num_speakers": 2,
    }


def test_invalid_local_whisper_host_output_becomes_runtime_failure(tmp_path: Path) -> None:
    """Malformed local Whisper host output should be classified as runtime failure."""

    def runner(_audio: np.ndarray, lang: str) -> dict[str, object]:
        return {
            "lang": lang,
            "text": "hello world",
            "chunks": [
                {"text": "hello", "start_s": 0.6, "end_s": 0.2},
            ],
        }

    response = execute_asr_request_v2(
        _make_request(tmp_path),
        AsrExecutionHostV2(local_whisper_runner=runner),  # type: ignore[arg-type]
    )

    assert isinstance(response.outcome, ExecuteErrorV2)
    assert response.outcome.code is ProtocolErrorCodeV2.RUNTIME_FAILURE
    assert "invalid ASR host output" in response.outcome.message
    assert response.result is None


def test_invalid_provider_asr_host_output_becomes_runtime_failure() -> None:
    """Malformed provider ASR host output should be classified as runtime failure."""

    def runner(_item) -> MonologueAsrResponse:
        return MonologueAsrResponse(
            lang="yue",
            monologues=[
                AsrMonologue(
                    speaker=1,
                    elements=[
                        AsrElement(
                            value="nei5",
                            ts=0.8,
                            end_ts=0.4,
                            type="text",
                        )
                    ],
                )
            ],
        )

    response = execute_asr_request_v2(
        _make_provider_request(),
        AsrExecutionHostV2(hk_tencent_runner=runner),
    )

    assert isinstance(response.outcome, ExecuteErrorV2)
    assert response.outcome.code is ProtocolErrorCodeV2.RUNTIME_FAILURE
    assert "invalid ASR host output" in response.outcome.message
    assert response.result is None


def test_whisper_chunk_inverted_timestamps_are_clamped(tmp_path: Path) -> None:
    """Whisper occasionally returns chunks with end_s < start_s on long audio.

    The inference layer must swap them rather than letting the Pydantic
    validator reject the entire response. Regression test for job 696870c7-02b
    (maria16.wav).
    """

    def runner(audio: np.ndarray, lang: str) -> WhisperChunkResultPayloadV2:
        # The clamping happens in infer_whisper_prepared_audio *before*
        # building WhisperChunkSpanV2 objects. This test verifies the V2
        # executor accepts already-clamped data (i.e. swapped to valid range).
        return WhisperChunkResultPayloadV2(
            lang=lang,
            text=" Thank you.",
            chunks=[
                WhisperChunkSpanV2(text=" Thank you.", start_s=2017.0, end_s=2020.0),
            ],
        )

    response = execute_asr_request_v2(
        _make_request(tmp_path),
        AsrExecutionHostV2(local_whisper_runner=runner),
    )

    assert isinstance(response.outcome, ExecuteSuccessV2)
    assert isinstance(response.result, WhisperChunkResultV2)
    assert response.result.chunks[0].start_s == 2017.0
    assert response.result.chunks[0].end_s == 2020.0


def test_whisper_chunk_span_v2_rejects_inverted_timestamps() -> None:
    """Verify the Pydantic validator catches inverted timestamps.

    This is the safety net — if the clamping in infer_whisper_prepared_audio
    is ever bypassed, the validator must reject.
    """
    import pytest

    with pytest.raises(Exception, match="end_s must be >= start_s"):
        WhisperChunkSpanV2(text="bad", start_s=2020.0, end_s=2017.0)
