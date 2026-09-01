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
    assert backend.name == "pyannote-ai:precision-2:speakers-2:v4"
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


def test_default_transport_reuses_one_connection_pool():
    from batchalign.backends.speaker.pyannote_ai import _PooledUrlOpen

    calls = []

    class FakePool:
        def request(self, method, url, **kwargs):
            calls.append((method, url, kwargs))
            response = _Response(b"ok")
            response.status = 200
            return response

    opener = _PooledUrlOpen(FakePool())
    first = urllib.request.Request("https://api.test/one", method="GET")
    second = urllib.request.Request(
        "https://upload.test/two",
        data=b"payload",
        headers={"Content-Type": "application/octet-stream"},
        method="PUT",
    )

    with opener(first, timeout=7.0) as response:
        assert response.read() == b"ok"
    with opener(second, timeout=11.0) as response:
        assert response.read() == b"ok"

    assert [call[:2] for call in calls] == [
        ("GET", "https://api.test/one"),
        ("PUT", "https://upload.test/two"),
    ]
    assert calls[0][2]["timeout"] == 7.0
    assert calls[1][2]["body"] == b"payload"
    headers = {key.lower(): value for key, value in calls[1][2]["headers"].items()}
    assert headers["content-type"] == "application/octet-stream"
    assert all(call[2]["redirect"] is True for call in calls)
    assert all(call[2]["preload_content"] is False for call in calls)


def test_pooled_transport_preserves_rate_limit_retries():
    from batchalign.backends.speaker.pyannote_ai import (
        PyannoteAIBackend,
        _PooledUrlOpen,
    )

    calls = 0
    delays = []

    class FakePool:
        def request(self, _method, _url, **_kwargs):
            nonlocal calls
            calls += 1
            if calls == 1:
                response = _Response(b'{"message":"slow down"}')
                response.status = 429
                response.reason = "Too Many Requests"
                response.headers = {"Retry-After": "2"}
                return response
            response = _Response(b"ok")
            response.status = 200
            return response

    backend = PyannoteAIBackend(
        api_key="secret",
        urlopen=_PooledUrlOpen(FakePool()),
        sleep=delays.append,
    )
    request = urllib.request.Request("https://api.test/job", method="GET")

    assert backend._open(request, operation="poll job") == b"ok"
    assert calls == 2
    assert delays == [2.0]


def test_native_converter_renders_compact_mp3():
    from batchalign.backends.speaker.pyannote_ai import _render_mp3

    rendered = _render_mp3(_audio())
    assert rendered[0] == 0xFF
    assert rendered[1] & 0xE0 == 0xE0
