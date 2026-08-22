"""pyannoteAI cloud backend tests with the HTTP boundary fully faked."""

from __future__ import annotations

import base64
import io
import json
from typing import Any


class _Response(io.BytesIO):
    def __enter__(self):
        return self

    def __exit__(self, *_args: Any) -> None:
        self.close()


def _audio():
    from batchalign._core.proto import PreparedAudio

    return PreparedAudio(
        pcm_f32le=base64.b64encode(b"\x00" * 64).decode(),
        sample_rate=16_000,
        channels=1,
        frame_count=16,
    )


def test_diarization_upload_submit_poll_and_projection():
    from batchalign._core.proto import SpeakerInput
    from batchalign.backends.speaker.pyannote_ai import PyannoteAIBackend

    requests = []

    def urlopen(request, *, timeout):
        requests.append((request, timeout))
        method = request.get_method()
        url = request.full_url
        if method == "POST" and url.endswith("/v1/media/input"):
            return _Response(json.dumps({"url": "https://upload.test/wav"}).encode())
        if method == "PUT" and url == "https://upload.test/wav":
            return _Response(b"")
        if method == "POST" and url.endswith("/v1/diarize"):
            return _Response(json.dumps({"jobId": "job-1", "status": "created"}).encode())
        if method == "GET" and url.endswith("/v1/jobs/job-1"):
            return _Response(
                json.dumps(
                    {
                        "jobId": "job-1",
                        "status": "succeeded",
                        "output": {
                            "diarization": [
                                {"speaker": "SPEAKER_01", "start": 1.25, "end": 2.5},
                                {"speaker": "SPEAKER_00", "start": 0.1, "end": 0.8},
                            ]
                        },
                    }
                ).encode()
            )
        raise AssertionError(f"unexpected request: {method} {url}")

    rendered = []

    def render_wav(audio):
        rendered.append(audio)
        return b"RIFF-from-convert-backend"

    backend = PyannoteAIBackend(
        api_key="secret",
        num_speakers=2,
        urlopen=urlopen,
        wav_renderer=render_wav,
        sleep=lambda _seconds: None,
    )
    output = backend.call(
        [SpeakerInput(source_id="clip", audio=_audio(), num_speakers=0)]
    )[0]

    assert rendered and rendered[0].sample_rate == 16_000
    assert [request.get_method() for request, _ in requests] == [
        "POST",
        "PUT",
        "POST",
        "GET",
    ]
    assert requests[1][0].data == b"RIFF-from-convert-backend"
    assert requests[1][0].get_header("Authorization") is None
    submitted = json.loads(requests[2][0].data)
    assert submitted == {
        "url": json.loads(requests[0][0].data)["url"],
        "model": "precision-2",
        "numSpeakers": 2,
    }
    assert [
        (segment.start_ms, segment.end_ms, segment.speaker)
        for segment in output.diarization.segments
    ] == [
        (100, 800, "SPEAKER_00"),
        (1250, 2500, "SPEAKER_01"),
    ]


def test_missing_key_fails_before_upload():
    import pytest

    from batchalign._core.proto import SpeakerInput
    from batchalign.backends.speaker.pyannote_ai import PyannoteAIBackend

    backend = PyannoteAIBackend(api_key="", wav_renderer=lambda _audio: b"")
    with pytest.raises(RuntimeError, match=r"\[diarize\].*engine\.pyannote\.key"):
        backend.call([SpeakerInput(source_id="clip", audio=_audio())])


def test_native_converter_renders_wav():
    from batchalign.backends.speaker.pyannote_ai import _render_wav

    rendered = _render_wav(_audio())
    assert rendered[:4] == b"RIFF"
    assert rendered[8:12] == b"WAVE"
