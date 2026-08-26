from __future__ import annotations

import wave
from types import SimpleNamespace

from typer.testing import CliRunner

from batchalign.cli import app

from batchalign.cli.diarize import (
    DiarizeEngine,
    _build_backend,
)


def test_backend_selector_defaults_to_cloud_and_can_select_local():
    calls = []
    cloud = object()
    local = object()
    ba = SimpleNamespace(
        PyannoteAIBackend=lambda **kwargs: calls.append(("cloud", kwargs)) or cloud,
        PyannoteBackend=lambda **kwargs: calls.append(("local", kwargs)) or local,
    )

    assert _build_backend(ba, DiarizeEngine.pyannote_ai, 2) is cloud
    assert _build_backend(ba, DiarizeEngine.pyannote, 3) is local
    assert calls == [
        ("cloud", {"num_speakers": 2}),
        ("local", {"num_speakers": 3}),
    ]


def test_diarize_runs_shared_pipeline_on_chat_and_writes_chat(
    tmp_path, monkeypatch,
):
    import batchalign as ba

    chat = tmp_path / "sample.cha"
    chat.write_text(
        "@Begin\n@Languages:\teng\n@Participants:\tPAR Participant\n"
        "@ID:\teng|batchalign|PAR|||||Participant|||\n"
        "*PAR:\thello .\n@End\n",
        encoding="utf-8",
    )
    captured = {}
    backend = object()
    monkeypatch.setattr(ba, "PyannoteAIBackend", lambda **_kwargs: backend)

    class FakeOutcome:
        source_id = str(chat)

        def write(self, target, *, strip_word_timing=False):
            captured["target"] = target
            captured["strip_word_timing"] = strip_word_timing

    class FakePipeline:
        def run(self, inputs, callbacks=None, outcome_callback=None):
            captured["inputs"] = list(inputs)
            outcome = FakeOutcome()
            if outcome_callback is not None:
                outcome_callback(outcome)
            return [outcome]

    def fake_recipe(**kwargs):
        captured["recipe"] = kwargs
        return FakePipeline()

    monkeypatch.setattr(ba.recipes, "diarize", fake_recipe)

    result = CliRunner().invoke(app, ["diarize", str(chat)])

    assert result.exit_code == 0, result.output
    assert captured["recipe"]["speaker_backend"] is backend
    assert len(captured["inputs"]) == 1
    assert str(captured["inputs"][0].path) == str(chat)
    assert captured["target"] == str(chat)
    assert captured["strip_word_timing"] is False


def test_diarize_cloud_cli_relabels_timed_chat_end_to_end(
    tmp_path, monkeypatch,
):
    """Exercise the real CLI, recipe, Rust runner, and CHAT writer."""
    import batchalign as ba
    from batchalign.backends.base import BatchPolicy
    from batchalign._core.proto import (
        Diarization,
        DiarizationSegment,
        SpeakerOutput,
    )

    chat = tmp_path / "sample.cha"
    chat.write_text(
        "@UTF8\n@Begin\n@Languages:\teng\n"
        "@Participants:\tPAR0 Participant, PAR1 Participant\n"
        "@ID:\teng|batchalign|PAR0|||||Participant|||\n"
        "@ID:\teng|batchalign|PAR1|||||Participant|||\n"
        "@Media:\tsample, audio\n"
        "*PAR0:\thello . \x150_400\x15\n"
        "*PAR0:\tgoodbye . \x15600_1000\x15\n"
        "@End\n",
        encoding="utf-8",
    )
    with wave.open(str(tmp_path / "sample.wav"), "wb") as wav:
        wav.setnchannels(1)
        wav.setsampwidth(2)
        wav.setframerate(16_000)
        wav.writeframes(b"\x00\x00" * 16_000)

    class FakeCloudSpeaker(ba.Speaker):
        @property
        def name(self):
            return "fake-pyannote-cloud"

        @property
        def batch_policy(self):
            return BatchPolicy.one()

        def call(self, batch, **_kwargs):
            return [
                SpeakerOutput(
                    source_id=item.source_id,
                    diarization=Diarization(
                        segments=[
                            DiarizationSegment(
                                start_ms=0, end_ms=500, speaker="speaker-a"
                            ),
                            DiarizationSegment(
                                start_ms=501, end_ms=1000, speaker="speaker-b"
                            ),
                        ]
                    ),
                )
                for item in batch
            ]

    monkeypatch.setattr(
        ba, "PyannoteAIBackend", lambda **_kwargs: FakeCloudSpeaker()
    )
    output = tmp_path / "out"
    result = CliRunner().invoke(
        app,
        [
            "--workers", "1", "diarize", str(chat),
            "--out", str(output), "--engine", "pyannote-ai",
        ],
    )

    assert result.exit_code == 0, result.output
    rendered = (output / "sample.cha").read_text(encoding="utf-8")
    assert "speaker: fake-pyannote-cloud" in rendered
    assert "*PAR0:\thello" in rendered
    assert "*PAR1:\tgoodbye" in rendered
