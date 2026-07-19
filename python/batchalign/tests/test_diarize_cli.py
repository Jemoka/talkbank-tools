from __future__ import annotations

from types import SimpleNamespace

from batchalign.cli.diarize import _turns_document


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
        "source": "batchalign3:pyannote",
        "turns": [
            {"start_ms": 100, "end_ms": 250, "track": "PAR1"},
            {"start_ms": 260, "end_ms": 500, "track": "PAR0"},
            {"start_ms": 510, "end_ms": 700, "track": "PAR1"},
        ],
    }
