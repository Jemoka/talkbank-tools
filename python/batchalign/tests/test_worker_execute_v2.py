# affects: python/batchalign/worker/_execute_v2.py
# affects: python/batchalign/worker/_types_v2.py
"""Tests for the live worker-protocol V2 execution router."""

from __future__ import annotations

import json
from pathlib import Path

import numpy as np

from batchalign.worker._execute_v2 import WorkerExecutionHostV2, execute_request_v2
from batchalign.worker._asr_v2 import AsrExecutionHostV2
from batchalign.worker._avqi_v2 import AvqiExecutionHostV2
from batchalign.worker._fa_v2 import ForcedAlignmentExecutionHostV2
from batchalign.worker._opensmile_v2 import OpenSmileExecutionHostV2
from batchalign.worker._speaker_v2 import SpeakerExecutionHostV2
from batchalign.worker._text_v2 import TextExecutionHostV2
from batchalign.worker._types import _state
from batchalign.inference.asr import AsrElement, AsrMonologue, MonologueAsrResponse
from batchalign.inference.avqi import AvqiResponse
from batchalign.inference.opensmile import OpenSmileResponse
from batchalign.inference.speaker import SpeakerResponse, SpeakerSegment
from batchalign.worker._types import BatchInferRequest, BatchInferResponse, InferResponse
from batchalign.worker._types_v2 import (
    AvqiRequestV2,
    AvqiResultV2,
    AsrBackendV2,
    AsrRequestV2,
    ExecuteErrorV2,
    ExecuteRequestV2,
    ExecuteSuccessV2,
    FaBackendV2,
    FaTextModeV2,
    ForcedAlignmentRequestV2,
    InferenceTaskV2,
    MonologueAsrResultV2,
    CorefRequestV2,
    CorefResultV2,
    MorphosyntaxRequestV2,
    MorphosyntaxResultV2,
    OpenSmileRequestV2,
    OpenSmileResultV2,
    PreparedAudioEncodingV2,
    PreparedAudioInputV2,
    PreparedAudioRefV2,
    PreparedTextEncodingV2,
    PreparedTextRefV2,
    ProviderMediaInputV2,
    ProtocolErrorCodeV2,
    SpeakerBackendV2,
    SpeakerPreparedAudioInputV2,
    SpeakerRequestV2,
    SpeakerResultV2,
    TranslateRequestV2,
    TranslationResultV2,
    UtsegRequestV2,
    UtsegResultV2,
    WhisperChunkResultPayloadV2,
    WhisperChunkResultV2,
    WhisperChunkSpanV2,
    WhisperTokenTimingResultV2,
)


def _write_pcm_f32le(path: Path, samples: np.ndarray) -> None:
    """Write little-endian float32 PCM test data to disk."""

    path.write_bytes(samples.astype("<f4").tobytes())


def _write_fa_payload(path: Path) -> None:
    """Write a minimal prepared FA payload for live V2 router tests."""

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


def _write_json_payload(path: Path, value: object) -> None:
    """Write one prepared JSON payload for live V2 router tests."""

    path.write_text(json.dumps(value), encoding="utf-8")


def _assert_runtime_failure_response(response, message_fragment: str) -> None:
    """Assert one typed runtime-failure response from the V2 router."""

    assert isinstance(response.outcome, ExecuteErrorV2)
    assert response.outcome.code is ProtocolErrorCodeV2.RUNTIME_FAILURE
    assert message_fragment in response.outcome.message
    assert response.result is None


def test_test_echo_rejects_mismatched_task_payload_boundary() -> None:
    """Test-echo mode must still reject malformed top-level V2 task/payload pairs."""

    previous_test_echo = _state.test_echo
    previous_test_delay_ms = _state.test_delay_ms
    _state.test_echo = True
    _state.test_delay_ms = 0
    try:
        response = execute_request_v2(
            request=ExecuteRequestV2(
                request_id="req-execute-v2-mismatch-1",
                task=InferenceTaskV2.MORPHOSYNTAX,
                payload=AsrRequestV2(
                    lang="eng",
                    backend=AsrBackendV2.HK_TENCENT,
                    input=ProviderMediaInputV2(
                        media_path="/tmp/provider.wav",
                        num_speakers=2,
                    ),
                ),
                attachments=[],
            )
        )
    finally:
        _state.test_echo = previous_test_echo
        _state.test_delay_ms = previous_test_delay_ms

    assert isinstance(response.outcome, ExecuteErrorV2)
    assert response.outcome.code is ProtocolErrorCodeV2.INVALID_PAYLOAD
    assert "does not match task morphosyntax" in response.outcome.message
    assert response.result is None


def _make_fa_request(tmp_path: Path) -> (
    tuple[
        PreparedTextRefV2,
        PreparedAudioRefV2,
        ForcedAlignmentTaskRequestV2,
    ]
):
    """Build the attachments and payload wrapper for one FA V2 request."""

    payload_path = tmp_path / "payload.json"
    audio_path = tmp_path / "audio.pcm"
    _write_fa_payload(payload_path)
    _write_pcm_f32le(audio_path, np.asarray([0.1, 0.2, 0.3, 0.4], dtype=np.float32))

    return (
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
        ForcedAlignmentRequestV2(
            backend=FaBackendV2.WHISPER,
            payload_ref_id="payload-ref-1",
            audio_ref_id="audio-ref-1",
            text_mode=FaTextModeV2.SPACE_JOINED,
            pauses=True,
        ),
    )


def test_routes_forced_alignment_execute_v2_request(tmp_path: Path) -> None:
    """The live V2 router should hand FA requests to the staged FA executor."""

    payload_attachment, audio_attachment, payload = _make_fa_request(tmp_path)

    response = execute_request_v2(
        request=ExecuteRequestV2(
            request_id="req-execute-v2-fa-1",
            task=InferenceTaskV2.FORCED_ALIGNMENT,
            payload=payload,
            attachments=[payload_attachment, audio_attachment],
        ),
        host=WorkerExecutionHostV2(
            asr=AsrExecutionHostV2(),
            forced_alignment=ForcedAlignmentExecutionHostV2(
                whisper_runner=lambda audio, text, pauses: [
                    ("hello", 0.1 if audio.shape == (4,) else 0.0),
                    ("world", 0.3 if text == "hello world" and pauses else 0.0),
                ]
            ),
            speaker=SpeakerExecutionHostV2(),
        ),
    )

    assert isinstance(response.outcome, ExecuteSuccessV2)
    assert isinstance(response.result, WhisperTokenTimingResultV2)
    assert response.result.tokens[0].text == "hello"
    assert response.result.tokens[1].time_s == 0.3


def test_routes_asr_execute_v2_request(tmp_path: Path) -> None:
    """The live V2 router should hand ASR requests to the ASR executor."""

    _payload_attachment, audio_attachment, _payload = _make_fa_request(tmp_path)

    response = execute_request_v2(
        request=ExecuteRequestV2(
            request_id="req-execute-v2-asr-1",
            task=InferenceTaskV2.ASR,
            payload=AsrRequestV2(
                lang="eng",
                backend=AsrBackendV2.LOCAL_WHISPER,
                input=PreparedAudioInputV2(audio_ref_id="audio-ref-1"),
            ),
            attachments=[audio_attachment],
        ),
        host=WorkerExecutionHostV2(
            asr=AsrExecutionHostV2(
                local_whisper_runner=lambda audio, lang: WhisperChunkResultPayloadV2(
                    lang=lang,
                    text="hello",
                    chunks=[
                        WhisperChunkSpanV2(
                            text="hello",
                            start_s=0.0 if audio.shape == (4,) else 1.0,
                            end_s=0.5,
                        )
                    ],
                )
            ),
            forced_alignment=ForcedAlignmentExecutionHostV2(),
            speaker=SpeakerExecutionHostV2(),
        ),
    )

    assert isinstance(response.outcome, ExecuteSuccessV2)
    assert isinstance(response.result, WhisperChunkResultV2)
    assert response.result.text == "hello"


def test_routes_provider_media_asr_execute_v2_request() -> None:
    """The live V2 router should hand provider-media ASR requests to ASR hosts."""

    response = execute_request_v2(
        request=ExecuteRequestV2(
            request_id="req-execute-v2-asr-provider-1",
            task=InferenceTaskV2.ASR,
            payload=AsrRequestV2(
                lang="yue",
                backend=AsrBackendV2.HK_TENCENT,
                input=ProviderMediaInputV2(
                    media_path="/tmp/provider.wav",
                    num_speakers=2,
                ),
            ),
            attachments=[],
        ),
        host=WorkerExecutionHostV2(
            asr=AsrExecutionHostV2(
                hk_tencent_runner=lambda item: MonologueAsrResponse(
                    lang=item.lang,
                    monologues=[
                        AsrMonologue(
                            speaker=item.num_speakers - 1,
                            elements=[AsrElement(value=item.audio_path, type="text")],
                        )
                    ],
                )
            ),
            forced_alignment=ForcedAlignmentExecutionHostV2(),
            speaker=SpeakerExecutionHostV2(),
        ),
    )

    assert isinstance(response.outcome, ExecuteSuccessV2)
    assert isinstance(response.result, MonologueAsrResultV2)
    assert response.result.monologues[0].speaker == "1"
    assert response.result.monologues[0].elements[0].value == "/tmp/provider.wav"


def test_routes_speaker_execute_v2_request(tmp_path: Path) -> None:
    """The live V2 router should hand speaker requests to the speaker host."""

    audio_path = tmp_path / "speaker-audio.pcm"
    _write_pcm_f32le(audio_path, np.asarray([0.2, 0.1, 0.0, -0.1], dtype=np.float32))
    response = execute_request_v2(
        request=ExecuteRequestV2(
            request_id="req-execute-v2-speaker-1",
            task=InferenceTaskV2.SPEAKER,
            payload=SpeakerRequestV2(
                backend=SpeakerBackendV2.PYANNOTE,
                input=SpeakerPreparedAudioInputV2(audio_ref_id="audio-ref-speaker-1"),
                expected_speakers=3,
            ),
            attachments=[
                PreparedAudioRefV2(
                    id="audio-ref-speaker-1",
                    path=str(audio_path),
                    encoding=PreparedAudioEncodingV2.PCM_F32LE,
                    channels=1,
                    sample_rate_hz=16000,
                    frame_count=4,
                    byte_offset=0,
                    byte_len=16,
                )
            ],
        ),
        host=WorkerExecutionHostV2(
            asr=AsrExecutionHostV2(),
            forced_alignment=ForcedAlignmentExecutionHostV2(),
            speaker=SpeakerExecutionHostV2(
                pyannote_prepared_audio_runner=lambda audio, sample_rate_hz, num_speakers: SpeakerResponse(
                    segments=[
                        SpeakerSegment(
                            start_ms=0,
                            end_ms=1200,
                            speaker=f"SPEAKER_{num_speakers}_{sample_rate_hz}_{audio.shape[0]}",
                        )
                    ]
                )
            ),
        ),
    )

    assert isinstance(response.outcome, ExecuteSuccessV2)
    assert isinstance(response.result, SpeakerResultV2)
    assert response.result.segments[0].speaker == "SPEAKER_3_16000_4"
    assert response.result.segments[0].end_ms == 1200


def test_routes_opensmile_execute_v2_request(tmp_path: Path) -> None:
    """The live V2 router should hand openSMILE requests to the staged executor."""

    audio_path = tmp_path / "opensmile-audio.pcm"
    _write_pcm_f32le(audio_path, np.asarray([0.0, 0.5, -0.5, 0.25], dtype=np.float32))

    response = execute_request_v2(
        request=ExecuteRequestV2(
            request_id="req-execute-v2-opensmile-1",
            task=InferenceTaskV2.OPENSMILE,
            payload=OpenSmileRequestV2(
                audio_ref_id="audio-ref-opensmile-1",
                feature_set="eGeMAPSv02",
                feature_level="functionals",
            ),
            attachments=[
                PreparedAudioRefV2(
                    id="audio-ref-opensmile-1",
                    path=str(audio_path),
                    encoding=PreparedAudioEncodingV2.PCM_F32LE,
                    channels=1,
                    sample_rate_hz=16000,
                    frame_count=4,
                    byte_offset=0,
                    byte_len=16,
                )
            ],
        ),
        host=WorkerExecutionHostV2(
            opensmile=OpenSmileExecutionHostV2(
                prepared_audio_runner=lambda audio, sample_rate_hz, feature_set, feature_level, audio_label: OpenSmileResponse(
                    feature_set=feature_set,
                    feature_level=feature_level,
                    num_features=2 if sample_rate_hz == 16000 else 0,
                    duration_segments=1 if audio.shape == (4,) else 0,
                    audio_file=audio_label,
                    rows=[{"f0_mean": 100.0, "jitter_local": 0.1}],
                    success=True,
                )
            ),
        ),
    )

    assert isinstance(response.outcome, ExecuteSuccessV2)
    assert isinstance(response.result, OpenSmileResultV2)
    assert response.result.rows[0]["f0_mean"] == 100.0
    assert response.result.feature_set == "eGeMAPSv02"


def test_invalid_opensmile_host_output_becomes_runtime_failure(tmp_path: Path) -> None:
    """Malformed openSMILE host output should be classified as runtime failure."""

    audio_path = tmp_path / "opensmile-invalid-audio.pcm"
    _write_pcm_f32le(audio_path, np.asarray([0.0, 0.5, -0.5, 0.25], dtype=np.float32))

    response = execute_request_v2(
        request=ExecuteRequestV2(
            request_id="req-execute-v2-opensmile-invalid-1",
            task=InferenceTaskV2.OPENSMILE,
            payload=OpenSmileRequestV2(
                audio_ref_id="audio-ref-opensmile-invalid-1",
                feature_set="eGeMAPSv02",
                feature_level="functionals",
            ),
            attachments=[
                PreparedAudioRefV2(
                    id="audio-ref-opensmile-invalid-1",
                    path=str(audio_path),
                    encoding=PreparedAudioEncodingV2.PCM_F32LE,
                    channels=1,
                    sample_rate_hz=16000,
                    frame_count=4,
                    byte_offset=0,
                    byte_len=16,
                )
            ],
        ),
        host=WorkerExecutionHostV2(
            opensmile=OpenSmileExecutionHostV2(
                prepared_audio_runner=lambda *_args: OpenSmileResponse(
                    feature_set="eGeMAPSv02",
                    feature_level="functionals",
                    num_features=1,
                    duration_segments=1,
                    audio_file="invalid.wav",
                    rows=[{"f0_mean": float("nan")}],
                    success=True,
                )
            ),
        ),
    )

    _assert_runtime_failure_response(response, "invalid openSMILE host output")


def test_routes_avqi_execute_v2_request(tmp_path: Path) -> None:
    """The live V2 router should hand AVQI requests to the staged executor."""

    cs_path = tmp_path / "sample.cs.pcm"
    sv_path = tmp_path / "sample.sv.pcm"
    _write_pcm_f32le(cs_path, np.asarray([0.1, 0.2, 0.3, 0.4], dtype=np.float32))
    _write_pcm_f32le(sv_path, np.asarray([0.5, 0.6, 0.7, 0.8], dtype=np.float32))

    response = execute_request_v2(
        request=ExecuteRequestV2(
            request_id="req-execute-v2-avqi-1",
            task=InferenceTaskV2.AVQI,
            payload=AvqiRequestV2(
                cs_audio_ref_id="audio-ref-avqi-cs-1",
                sv_audio_ref_id="audio-ref-avqi-sv-1",
            ),
            attachments=[
                PreparedAudioRefV2(
                    id="audio-ref-avqi-cs-1",
                    path=str(cs_path),
                    encoding=PreparedAudioEncodingV2.PCM_F32LE,
                    channels=1,
                    sample_rate_hz=16000,
                    frame_count=4,
                    byte_offset=0,
                    byte_len=16,
                ),
                PreparedAudioRefV2(
                    id="audio-ref-avqi-sv-1",
                    path=str(sv_path),
                    encoding=PreparedAudioEncodingV2.PCM_F32LE,
                    channels=1,
                    sample_rate_hz=16000,
                    frame_count=4,
                    byte_offset=0,
                    byte_len=16,
                ),
            ],
        ),
        host=WorkerExecutionHostV2(
            avqi=AvqiExecutionHostV2(
                prepared_audio_runner=lambda cs_audio, cs_rate, sv_audio, sv_rate, cs_label, sv_label: AvqiResponse(
                    avqi=3.14 if cs_audio.shape == (4,) and sv_audio.shape == (4,) else 0.0,
                    cpps=5.0,
                    hnr=10.0,
                    shimmer_local=0.2,
                    shimmer_local_db=0.3,
                    slope=0.4,
                    tilt=0.5,
                    cs_file=cs_label if cs_rate == 16000 else "bad",
                    sv_file=sv_label if sv_rate == 16000 else "bad",
                    success=True,
                )
            ),
        ),
    )

    assert isinstance(response.outcome, ExecuteSuccessV2)
    assert isinstance(response.result, AvqiResultV2)
    assert response.result.avqi == 3.14
    assert response.result.cs_file.endswith("sample.cs.pcm")


def test_invalid_avqi_host_output_becomes_runtime_failure(tmp_path: Path) -> None:
    """Malformed AVQI host output should be classified as runtime failure."""

    cs_path = tmp_path / "invalid.cs.pcm"
    sv_path = tmp_path / "invalid.sv.pcm"
    _write_pcm_f32le(cs_path, np.asarray([0.1, 0.2, 0.3, 0.4], dtype=np.float32))
    _write_pcm_f32le(sv_path, np.asarray([0.5, 0.6, 0.7, 0.8], dtype=np.float32))

    response = execute_request_v2(
        request=ExecuteRequestV2(
            request_id="req-execute-v2-avqi-invalid-1",
            task=InferenceTaskV2.AVQI,
            payload=AvqiRequestV2(
                cs_audio_ref_id="audio-ref-avqi-invalid-cs-1",
                sv_audio_ref_id="audio-ref-avqi-invalid-sv-1",
            ),
            attachments=[
                PreparedAudioRefV2(
                    id="audio-ref-avqi-invalid-cs-1",
                    path=str(cs_path),
                    encoding=PreparedAudioEncodingV2.PCM_F32LE,
                    channels=1,
                    sample_rate_hz=16000,
                    frame_count=4,
                    byte_offset=0,
                    byte_len=16,
                ),
                PreparedAudioRefV2(
                    id="audio-ref-avqi-invalid-sv-1",
                    path=str(sv_path),
                    encoding=PreparedAudioEncodingV2.PCM_F32LE,
                    channels=1,
                    sample_rate_hz=16000,
                    frame_count=4,
                    byte_offset=0,
                    byte_len=16,
                ),
            ],
        ),
        host=WorkerExecutionHostV2(
            avqi=AvqiExecutionHostV2(
                prepared_audio_runner=lambda *_args: AvqiResponse(
                    avqi=float("inf"),
                    cpps=5.0,
                    hnr=10.0,
                    shimmer_local=0.2,
                    shimmer_local_db=0.3,
                    slope=0.4,
                    tilt=0.5,
                    cs_file="invalid.cs.pcm",
                    sv_file="invalid.sv.pcm",
                    success=True,
                )
            ),
        ),
    )

    _assert_runtime_failure_response(response, "invalid AVQI host output")


def test_routes_morphosyntax_execute_v2_request(tmp_path: Path) -> None:
    """The live V2 router should hand morphosyntax requests to the text host."""

    payload_path = tmp_path / "morphosyntax-batch.json"
    _write_json_payload(
        payload_path,
        {
            "items": [
                {
                    "words": ["I", "saw", "it"],
                    "terminator": ".",
                    "special_forms": [[None, None], [None, None], [None, None]],
                    "lang": "eng",
                }
            ],
            "mwt": {},
        },
    )

    response = execute_request_v2(
        request=ExecuteRequestV2(
            request_id="req-execute-v2-morphosyntax-1",
            task=InferenceTaskV2.MORPHOSYNTAX,
            payload=MorphosyntaxRequestV2(
                lang="eng",
                payload_ref_id="text-ref-morphosyntax-1",
                item_count=1,
            ),
            attachments=[
                PreparedTextRefV2(
                    id="text-ref-morphosyntax-1",
                    path=str(payload_path),
                    encoding=PreparedTextEncodingV2.UTF8_JSON,
                    byte_offset=0,
                    byte_len=payload_path.stat().st_size,
                )
            ],
        ),
        host=WorkerExecutionHostV2(
            asr=AsrExecutionHostV2(),
            forced_alignment=ForcedAlignmentExecutionHostV2(),
            speaker=SpeakerExecutionHostV2(),
            text=TextExecutionHostV2(
                morphosyntax_runner=lambda req: BatchInferResponse(
                    results=[
                        InferResponse(
                            result={
                                "raw_sentences": [
                                    [
                                        {
                                            "id": 1,
                                            "text": "I",
                                            "lemma": "I",
                                            "upos": "PRON",
                                            "head": 2,
                                            "deprel": "nsubj",
                                        },
                                        {
                                            "id": 2,
                                            "text": "saw",
                                            "lemma": "see",
                                            "upos": "VERB",
                                            "head": 0,
                                            "deprel": "root",
                                        },
                                        {
                                            "id": 3,
                                            "text": "it",
                                            "lemma": "it",
                                            "upos": "PRON",
                                            "head": 2,
                                            "deprel": "obj",
                                        },
                                    ]
                                ]
                            },
                            elapsed_s=0.0,
                        )
                    ]
                )
            ),
        ),
    )

    assert isinstance(response.outcome, ExecuteSuccessV2)
    assert isinstance(response.result, MorphosyntaxResultV2)
    assert response.result.items[0].raw_sentences is not None
    assert response.result.items[0].raw_sentences[0][1]["lemma"] == "see"


def test_routes_morphosyntax_unicode_special_forms_request(tmp_path: Path) -> None:
    """Unicode morphosyntax payloads should preserve special forms through the V2 router."""

    payload_path = tmp_path / "morphosyntax-unicode-batch.json"
    _write_json_payload(
        payload_path,
        {
            "items": [
                {
                    "words": ["我", "食", "緊", "飯"],
                    "terminator": "。",
                    "special_forms": [
                        [None, None],
                        ["食緊", "eat.PROG"],
                        [None, None],
                        [None, None],
                    ],
                    "lang": "yue",
                }
            ],
            "mwt": {"食緊": ["食", "緊"]},
        },
    )

    captured: dict[str, object] = {}

    def runner(req: BatchInferRequest) -> BatchInferResponse:
        captured["items"] = req.items
        captured["mwt"] = req.mwt
        return BatchInferResponse(
            results=[
                InferResponse(
                    result={
                        "raw_sentences": [
                            [
                                {
                                    "id": 1,
                                    "text": "我",
                                    "lemma": "我",
                                    "upos": "PRON",
                                    "head": 2,
                                    "deprel": "nsubj",
                                },
                                {
                                    "id": 2,
                                    "text": "食緊",
                                    "lemma": "食緊",
                                    "upos": "VERB",
                                    "head": 0,
                                    "deprel": "root",
                                },
                                {
                                    "id": 3,
                                    "text": "飯",
                                    "lemma": "飯",
                                    "upos": "NOUN",
                                    "head": 2,
                                    "deprel": "obj",
                                },
                            ]
                        ]
                    },
                    elapsed_s=0.0,
                )
            ]
        )

    response = execute_request_v2(
        request=ExecuteRequestV2(
            request_id="req-execute-v2-morphosyntax-unicode-1",
            task=InferenceTaskV2.MORPHOSYNTAX,
            payload=MorphosyntaxRequestV2(
                lang="yue",
                payload_ref_id="text-ref-morphosyntax-unicode-1",
                item_count=1,
            ),
            attachments=[
                PreparedTextRefV2(
                    id="text-ref-morphosyntax-unicode-1",
                    path=str(payload_path),
                    encoding=PreparedTextEncodingV2.UTF8_JSON,
                    byte_offset=0,
                    byte_len=payload_path.stat().st_size,
                )
            ],
        ),
        host=WorkerExecutionHostV2(
            text=TextExecutionHostV2(morphosyntax_runner=runner),
        ),
    )

    assert isinstance(response.outcome, ExecuteSuccessV2)
    assert isinstance(response.result, MorphosyntaxResultV2)
    assert response.result.items[0].raw_sentences[0][1]["text"] == "食緊"
    assert isinstance(captured["items"], list)
    assert captured["items"][0]["special_forms"][1] == ["食緊", "eat.PROG"]
    assert captured["mwt"] == {"食緊": ["食", "緊"]}


def test_invalid_morphosyntax_host_output_becomes_runtime_failure(tmp_path: Path) -> None:
    """Malformed morphosyntax host output should be classified as runtime failure."""

    payload_path = tmp_path / "morphosyntax-invalid-batch.json"
    _write_json_payload(
        payload_path,
        {
            "items": [
                {
                    "words": ["hello"],
                    "terminator": ".",
                    "special_forms": [[None, None]],
                    "lang": "eng",
                }
            ],
            "mwt": {},
        },
    )

    response = execute_request_v2(
        request=ExecuteRequestV2(
            request_id="req-execute-v2-morphosyntax-invalid-1",
            task=InferenceTaskV2.MORPHOSYNTAX,
            payload=MorphosyntaxRequestV2(
                lang="eng",
                payload_ref_id="text-ref-morphosyntax-invalid-1",
                item_count=1,
            ),
            attachments=[
                PreparedTextRefV2(
                    id="text-ref-morphosyntax-invalid-1",
                    path=str(payload_path),
                    encoding=PreparedTextEncodingV2.UTF8_JSON,
                    byte_offset=0,
                    byte_len=payload_path.stat().st_size,
                )
            ],
        ),
        host=WorkerExecutionHostV2(
            text=TextExecutionHostV2(
                morphosyntax_runner=lambda _req: BatchInferResponse(
                    results=[InferResponse(result={"raw_sentences": "bad"}, elapsed_s=0.0)]
                )
            ),
        ),
    )

    _assert_runtime_failure_response(response, "invalid morphosyntax host output")


def test_routes_utseg_execute_v2_request(tmp_path: Path) -> None:
    """The live V2 router should hand utseg requests to the text host."""

    payload_path = tmp_path / "utseg-batch.json"
    _write_json_payload(
        payload_path,
        {"items": [{"words": ["hello", "world"], "text": "hello world"}]},
    )

    response = execute_request_v2(
        request=ExecuteRequestV2(
            request_id="req-execute-v2-utseg-1",
            task=InferenceTaskV2.UTSEG,
            payload=UtsegRequestV2(
                lang="eng",
                payload_ref_id="text-ref-utseg-1",
                item_count=1,
            ),
            attachments=[
                PreparedTextRefV2(
                    id="text-ref-utseg-1",
                    path=str(payload_path),
                    encoding=PreparedTextEncodingV2.UTF8_JSON,
                    byte_offset=0,
                    byte_len=payload_path.stat().st_size,
                )
            ],
        ),
        host=WorkerExecutionHostV2(
            asr=AsrExecutionHostV2(),
            forced_alignment=ForcedAlignmentExecutionHostV2(),
            speaker=SpeakerExecutionHostV2(),
            text=TextExecutionHostV2(
                utseg_runner=lambda req: BatchInferResponse(
                    results=[InferResponse(result={"trees": ["(ROOT (S hello world))"]}, elapsed_s=0.0)]
                )
            ),
        ),
    )

    assert isinstance(response.outcome, ExecuteSuccessV2)
    assert isinstance(response.result, UtsegResultV2)
    assert response.result.items[0].trees == ["(ROOT (S hello world))"]


def test_routes_utseg_execute_v2_request_with_assignments(tmp_path: Path) -> None:
    """The live V2 router should preserve direct utseg assignments."""

    payload_path = tmp_path / "utseg-batch-assignments.json"
    _write_json_payload(
        payload_path,
        {"items": [{"words": ["hello", "world"], "text": "hello world"}]},
    )

    response = execute_request_v2(
        request=ExecuteRequestV2(
            request_id="req-execute-v2-utseg-assignments-1",
            task=InferenceTaskV2.UTSEG,
            payload=UtsegRequestV2(
                lang="eng",
                payload_ref_id="text-ref-utseg-assignments-1",
                item_count=1,
            ),
            attachments=[
                PreparedTextRefV2(
                    id="text-ref-utseg-assignments-1",
                    path=str(payload_path),
                    encoding=PreparedTextEncodingV2.UTF8_JSON,
                    byte_offset=0,
                    byte_len=payload_path.stat().st_size,
                )
            ],
        ),
        host=WorkerExecutionHostV2(
            text=TextExecutionHostV2(
                utseg_runner=lambda req: BatchInferResponse(
                    results=[InferResponse(result={"assignments": [0, 1]}, elapsed_s=0.0)]
                )
            ),
        ),
    )

    assert isinstance(response.outcome, ExecuteSuccessV2)
    assert isinstance(response.result, UtsegResultV2)
    assert response.result.items[0].assignments == [0, 1]


def test_invalid_utseg_host_output_becomes_runtime_failure(tmp_path: Path) -> None:
    """Malformed utseg host output should be classified as runtime failure."""

    payload_path = tmp_path / "utseg-invalid-batch.json"
    _write_json_payload(payload_path, {"items": [{"words": ["hello"], "text": "hello"}]})

    response = execute_request_v2(
        request=ExecuteRequestV2(
            request_id="req-execute-v2-utseg-invalid-1",
            task=InferenceTaskV2.UTSEG,
            payload=UtsegRequestV2(
                lang="eng",
                payload_ref_id="text-ref-utseg-invalid-1",
                item_count=1,
            ),
            attachments=[
                PreparedTextRefV2(
                    id="text-ref-utseg-invalid-1",
                    path=str(payload_path),
                    encoding=PreparedTextEncodingV2.UTF8_JSON,
                    byte_offset=0,
                    byte_len=payload_path.stat().st_size,
                )
            ],
        ),
        host=WorkerExecutionHostV2(
            text=TextExecutionHostV2(
                utseg_runner=lambda _req: BatchInferResponse(
                    results=[InferResponse(result={"trees": [1, 2]}, elapsed_s=0.0)]
                )
            ),
        ),
    )

    _assert_runtime_failure_response(response, "invalid utseg host output")


def test_invalid_utseg_assignment_host_output_becomes_runtime_failure(tmp_path: Path) -> None:
    """Malformed utseg assignments should be classified as runtime failure."""

    payload_path = tmp_path / "utseg-invalid-assignments-batch.json"
    _write_json_payload(payload_path, {"items": [{"words": ["hello"], "text": "hello"}]})

    response = execute_request_v2(
        request=ExecuteRequestV2(
            request_id="req-execute-v2-utseg-invalid-assignments-1",
            task=InferenceTaskV2.UTSEG,
            payload=UtsegRequestV2(
                lang="eng",
                payload_ref_id="text-ref-utseg-invalid-assignments-1",
                item_count=1,
            ),
            attachments=[
                PreparedTextRefV2(
                    id="text-ref-utseg-invalid-assignments-1",
                    path=str(payload_path),
                    encoding=PreparedTextEncodingV2.UTF8_JSON,
                    byte_offset=0,
                    byte_len=payload_path.stat().st_size,
                )
            ],
        ),
        host=WorkerExecutionHostV2(
            text=TextExecutionHostV2(
                utseg_runner=lambda _req: BatchInferResponse(
                    results=[InferResponse(result={"assignments": ["bad"]}, elapsed_s=0.0)]
                )
            ),
        ),
    )

    _assert_runtime_failure_response(response, "invalid utseg host output")


def test_routes_translate_execute_v2_request(tmp_path: Path) -> None:
    """The live V2 router should hand translate requests to the text host."""

    payload_path = tmp_path / "translate-batch.json"
    _write_json_payload(payload_path, {"items": [{"text": "hello there"}]})

    response = execute_request_v2(
        request=ExecuteRequestV2(
            request_id="req-execute-v2-translate-1",
            task=InferenceTaskV2.TRANSLATE,
            payload=TranslateRequestV2(
                source_lang="eng",
                target_lang="spa",
                payload_ref_id="text-ref-translate-1",
                item_count=1,
            ),
            attachments=[
                PreparedTextRefV2(
                    id="text-ref-translate-1",
                    path=str(payload_path),
                    encoding=PreparedTextEncodingV2.UTF8_JSON,
                    byte_offset=0,
                    byte_len=payload_path.stat().st_size,
                )
            ],
        ),
        host=WorkerExecutionHostV2(
            asr=AsrExecutionHostV2(),
            forced_alignment=ForcedAlignmentExecutionHostV2(),
            speaker=SpeakerExecutionHostV2(),
            text=TextExecutionHostV2(
                translate_runner=lambda req: BatchInferResponse(
                    results=[InferResponse(result={"raw_translation": "hola"}, elapsed_s=0.0)]
                )
            ),
        ),
    )

    assert isinstance(response.outcome, ExecuteSuccessV2)
    assert isinstance(response.result, TranslationResultV2)
    assert response.result.items[0].raw_translation == "hola"


def test_invalid_translate_host_output_becomes_runtime_failure(tmp_path: Path) -> None:
    """Malformed translate host output should be classified as runtime failure."""

    payload_path = tmp_path / "translate-invalid-batch.json"
    _write_json_payload(payload_path, {"items": [{"text": "hello there"}]})

    response = execute_request_v2(
        request=ExecuteRequestV2(
            request_id="req-execute-v2-translate-invalid-1",
            task=InferenceTaskV2.TRANSLATE,
            payload=TranslateRequestV2(
                source_lang="eng",
                target_lang="spa",
                payload_ref_id="text-ref-translate-invalid-1",
                item_count=1,
            ),
            attachments=[
                PreparedTextRefV2(
                    id="text-ref-translate-invalid-1",
                    path=str(payload_path),
                    encoding=PreparedTextEncodingV2.UTF8_JSON,
                    byte_offset=0,
                    byte_len=payload_path.stat().st_size,
                )
            ],
        ),
        host=WorkerExecutionHostV2(
            text=TextExecutionHostV2(
                translate_runner=lambda _req: BatchInferResponse(
                    results=[InferResponse(result={"raw_translation": ["hola"]}, elapsed_s=0.0)]
                )
            ),
        ),
    )

    _assert_runtime_failure_response(response, "invalid translate host output")


def test_routes_coref_execute_v2_request(tmp_path: Path) -> None:
    """The live V2 router should hand coref requests to the text host."""

    payload_path = tmp_path / "coref-batch.json"
    _write_json_payload(payload_path, {"items": [{"sentences": [["she"]]}]})

    response = execute_request_v2(
        request=ExecuteRequestV2(
            request_id="req-execute-v2-coref-1",
            task=InferenceTaskV2.COREF,
            payload=CorefRequestV2(
                lang="eng",
                payload_ref_id="text-ref-coref-1",
                item_count=1,
            ),
            attachments=[
                PreparedTextRefV2(
                    id="text-ref-coref-1",
                    path=str(payload_path),
                    encoding=PreparedTextEncodingV2.UTF8_JSON,
                    byte_offset=0,
                    byte_len=payload_path.stat().st_size,
                )
            ],
        ),
        host=WorkerExecutionHostV2(
            asr=AsrExecutionHostV2(),
            forced_alignment=ForcedAlignmentExecutionHostV2(),
            speaker=SpeakerExecutionHostV2(),
            text=TextExecutionHostV2(
                coref_runner=lambda req: BatchInferResponse(
                    results=[
                        InferResponse(
                            result={
                                "annotations": [
                                    {
                                        "sentence_idx": 0,
                                        "words": [[{"chain_id": 0, "is_start": True, "is_end": True}]],
                                    }
                                ]
                            },
                            elapsed_s=0.0,
                        )
                    ]
                )
            ),
        ),
    )

    assert isinstance(response.outcome, ExecuteSuccessV2)
    assert isinstance(response.result, CorefResultV2)
    assert response.result.items[0].annotations is not None
    assert response.result.items[0].annotations[0].words[0][0].chain_id == 0


def test_invalid_coref_host_output_becomes_runtime_failure(tmp_path: Path) -> None:
    """Malformed coref host output should be classified as runtime failure."""

    payload_path = tmp_path / "coref-invalid-batch.json"
    _write_json_payload(payload_path, {"items": [{"sentences": [["she"]]}]})

    response = execute_request_v2(
        request=ExecuteRequestV2(
            request_id="req-execute-v2-coref-invalid-1",
            task=InferenceTaskV2.COREF,
            payload=CorefRequestV2(
                lang="eng",
                payload_ref_id="text-ref-coref-invalid-1",
                item_count=1,
            ),
            attachments=[
                PreparedTextRefV2(
                    id="text-ref-coref-invalid-1",
                    path=str(payload_path),
                    encoding=PreparedTextEncodingV2.UTF8_JSON,
                    byte_offset=0,
                    byte_len=payload_path.stat().st_size,
                )
            ],
        ),
        host=WorkerExecutionHostV2(
            text=TextExecutionHostV2(
                coref_runner=lambda _req: BatchInferResponse(
                    results=[InferResponse(result={"annotations": "bad"}, elapsed_s=0.0)]
                )
            ),
        ),
    )

    _assert_runtime_failure_response(response, "invalid coref host output")


def test_returns_typed_error_for_missing_text_v2_host(tmp_path: Path) -> None:
    """Text tasks without a live V2 host should fail explicitly."""

    payload_path = tmp_path / "unsupported-text-batch.json"
    _write_json_payload(payload_path, {"items": [{"text": "hello"}]})

    response = execute_request_v2(
        request=ExecuteRequestV2(
            request_id="req-execute-v2-unsupported-1",
            task=InferenceTaskV2.TRANSLATE,
            payload=TranslateRequestV2(
                source_lang="eng",
                target_lang="spa",
                payload_ref_id="text-ref-unsupported-1",
                item_count=1,
            ),
            attachments=[
                PreparedTextRefV2(
                    id="text-ref-unsupported-1",
                    path=str(payload_path),
                    encoding=PreparedTextEncodingV2.UTF8_JSON,
                    byte_offset=0,
                    byte_len=payload_path.stat().st_size,
                )
            ],
        ),
        host=WorkerExecutionHostV2(
            asr=AsrExecutionHostV2(),
            forced_alignment=ForcedAlignmentExecutionHostV2(),
            speaker=SpeakerExecutionHostV2(),
            text=TextExecutionHostV2(),
        ),
    )

    assert isinstance(response.outcome, ExecuteErrorV2)
    assert response.outcome.code is ProtocolErrorCodeV2.MODEL_UNAVAILABLE
    assert response.result is None
    assert "no translate host loaded" in response.outcome.message
