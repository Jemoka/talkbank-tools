from __future__ import annotations

from types import SimpleNamespace

from batchalign.cli.diarize import _protocol_audio, _turns_document


def test_turns_document_maps_sorted_labels_to_anonymous_tracks():
    output = SimpleNamespace(
        diarization=SimpleNamespace(
            segments=[
                SimpleNamespace(start_ms=100, end_ms=250, speaker="SPEAKER_01"),
                SimpleNamespace(start_ms=260, end_ms=500, speaker="SPEAKER_00"),
                SimpleNamespace(start_ms=510, end_ms=700, speaker="SPEAKER_01"),
            ]
        )
    )

    assert _turns_document(output) == {
        "source": "batchalign3:pyannote-ai",
        "turns": [
            {"start_ms": 100, "end_ms": 250, "track": "PAR1"},
            {"start_ms": 260, "end_ms": 500, "track": "PAR0"},
            {"start_ms": 510, "end_ms": 700, "track": "PAR1"},
        ],
    }


def test_protocol_audio_bridges_native_prepared_audio():
    native = SimpleNamespace(
        pcm_f32le=b"\x00\x01\x02\x03",
        sample_rate=16_000,
        channels=1,
        frame_count=1,
    )

    audio = _protocol_audio(native)

    assert bytes(audio.pcm_f32le) == native.pcm_f32le
    assert audio.sample_rate == 16_000
    assert audio.channels == 1
    assert audio.frame_count == 1
