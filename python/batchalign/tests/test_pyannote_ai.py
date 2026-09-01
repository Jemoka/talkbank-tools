"""pyannoteAI cloud backend tests with the HTTP boundary fully faked."""

from __future__ import annotations

import base64
import io
import json
import urllib.error
import urllib.request
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

    def render_media(audio):
        rendered.append(audio)
        return b"\xff\xfb-from-convert-backend"

    backend = PyannoteAIBackend(
        api_key="secret",
        num_speakers=2,
        urlopen=urlopen,
        media_renderer=render_media,
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
    assert backend.name == "pyannote-ai:precision-2:speakers-2:v3"
    declared = json.loads(requests[0][0].data)
    assert declared["url"].endswith(".mp3")
    assert requests[1][0].data == b"\xff\xfb-from-convert-backend"
    assert requests[1][0].get_header("Authorization") is None
    submitted = json.loads(requests[2][0].data)
    assert submitted == {
        "url": declared["url"],
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

    backend = PyannoteAIBackend(api_key="", media_renderer=lambda _audio: b"")
    with pytest.raises(RuntimeError, match=r"\[diarize\].*engine\.pyannote\.key"):
        backend.call([SpeakerInput(source_id="clip", audio=_audio())])


def test_idempotent_upload_retries_transient_write_timeout():
    from batchalign.backends.speaker.pyannote_ai import PyannoteAIBackend

    attempts = 0
    delays = []

    def urlopen(_request, *, timeout):
        nonlocal attempts
        assert timeout == 60.0
        attempts += 1
        if attempts < 3:
            raise urllib.error.URLError(TimeoutError("write timed out"))
        return _Response(b"")

    backend = PyannoteAIBackend(
        api_key="secret",
        urlopen=urlopen,
        sleep=delays.append,
    )
    request = urllib.request.Request(
        "https://upload.test/wav",
        data=b"RIFF",
        method="PUT",
    )

    assert backend._open(request, operation="upload media") == b""
    assert attempts == 3
    assert delays == [1, 2]


def test_native_converter_renders_compact_mp3():
    from batchalign.backends.speaker.pyannote_ai import _render_mp3

    rendered = _render_mp3(_audio())
    assert rendered[0] == 0xFF
    assert rendered[1] & 0xE0 == 0xE0
