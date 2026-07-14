"""Google Gemini ASR backend tests (the network client is fully faked)."""

from __future__ import annotations

import base64
import json
from types import SimpleNamespace


def _audio():
    from batchalign._core.proto import PreparedAudio

    pcm = base64.b64encode(b"\x00" * 64).decode()
    return PreparedAudio(
        pcm_f32le=pcm,
        sample_rate=16_000,
        channels=1,
        frame_count=16,
    )


class _Files:
    def __init__(self) -> None:
        self.uploads: list[str] = []
        self.deletes: list[str] = []
        self.gets: list[str] = []

    def upload(self, *, file: str, config: dict):
        self.uploads.append(file)
        assert config == {"mime_type": "audio/wav"}
        return SimpleNamespace(
            name="files/test-audio",
            uri="https://example.test/audio",
            mime_type="audio/wav",
            state=SimpleNamespace(name="PROCESSING"),
        )

    def get(self, *, name: str):
        self.gets.append(name)
        return SimpleNamespace(
            name=name,
            uri="https://example.test/audio",
            mime_type="audio/wav",
            state=SimpleNamespace(name="ACTIVE"),
        )

    def delete(self, *, name: str) -> None:
        self.deletes.append(name)


class _Interactions:
    def __init__(self) -> None:
        self.calls: list[dict] = []

    def create(self, **kwargs):
        self.calls.append(kwargs)
        transcript = {
            "segments": [
                {
                    "start_timestamp": "00:00.100",
                    "end_timestamp": "00:00.800",
                    "speaker": "0",
                    "content": "¿Hello there?",
                },
                {
                    "start_timestamp": "00:00.900",
                    "end_timestamp": "00:01.200",
                    "speaker": "1",
                    "content": "hi",
                },
            ]
        }
        return SimpleNamespace(output_text=f"```json\n{json.dumps(transcript)}\n```")


class _Client:
    def __init__(self) -> None:
        self.files = _Files()
        self.interactions = _Interactions()


def test_google_atomic_asr_and_diarization_share_one_interaction():
    from batchalign._core.proto import AsrInput, AsrOptions, SpeakerInput
    from batchalign.backends.asr.google import GoogleGenAIBackend
    from batchalign.lang import LanguageCode

    client = _Client()
    backend = GoogleGenAIBackend(
        client=client,
        language=LanguageCode.from_str("eng"),
        num_speakers=2,
        file_timeout_s=2,
    )
    audio = _audio()
    outputs = backend.call(
        [
            AsrInput(
                source_id="sample",
                audio=audio,
                language={"kind": "code", "value": "eng"},
                options=AsrOptions(),
            ),
            SpeakerInput(source_id="sample", audio=audio, num_speakers=2),
        ]
    )

    assert len(client.interactions.calls) == 1
    assert client.interactions.calls[0]["model"] == "gemini-3.5-flash"
    assert "max_output_tokens" not in client.interactions.calls[0]
    assert client.interactions.calls[0]["input"][1]["mime_type"] == "audio/wav"
    assert "extra_body" not in client.interactions.calls[0]
    assert client.files.deletes == ["files/test-audio"]
    assert client.files.gets == ["files/test-audio"]
    assert [segment.speaker for segment in outputs[0].segments] == ["0", "1"]
    assert outputs[0].segments[0].text == "Hello there"
    assert outputs[0].segments[0].start_ms == 100
    assert outputs[0].segments[0].words == []
    assert [segment.speaker for segment in outputs[1].diarization.segments] == [
        "0",
        "1",
    ]


def test_google_asr_key_uses_batchalign_config_discovery(tmp_path):
    from batchalign import config

    path = tmp_path / "batchalign.ini"
    path.write_text("[asr]\nengine.google.key = GEMINI_SECRET\n")
    assert config.get_api_key("google_asr", path=path) == "GEMINI_SECRET"
