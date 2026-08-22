"""Smoke-test the Typer CLI: every subcommand's `--help` must succeed."""

from __future__ import annotations

import pytest
import typer
from typer.testing import CliRunner

from batchalign.cli import app


SUBCOMMANDS = [
    "transcribe",
    "align",
    "morphotag",
    "translate",
    "ai",
    "utseg",
    "compare",
    "convert",
    "diarize",
]


def test_root_help():
    runner = CliRunner()
    result = runner.invoke(app, ["--help"])
    assert result.exit_code == 0
    for cmd in SUBCOMMANDS:
        assert cmd in result.output, f"missing {cmd} in root --help"
    assert "--workers" in result.output


@pytest.mark.parametrize("cmd", SUBCOMMANDS)
def test_subcommand_help(cmd):
    runner = CliRunner()
    result = runner.invoke(app, [cmd, "--help"])
    assert result.exit_code == 0, result.output
    assert cmd in result.output.lower() or "usage" in result.output.lower()


def _help(cmd: str) -> str:
    # Collapse the Rich box-drawing wrapping AND ANSI styling so
    # option/value tokens that get split across lines or wrapped in
    # color escapes are still greppable as substrings.
    import re
    out = CliRunner().invoke(app, [cmd, "--help"]).output
    out = re.sub(r"\x1b\[[0-9;]*m", "", out)
    return "".join(ch for ch in out if ch not in "│─╭╮╰╯").replace("\n", " ")


def test_transcribe_exposes_all_ba2_asr_engines():
    # parity.md: every engine BA2 supported must be reachable. The
    # transcribe CLI must offer rev / whisper / openai (whisperx + vllm
    # were dropped; the cloud engines plus chatwhisper / funaudio /
    # qwen3 cover the remaining surface).
    help_text = _help("transcribe")
    for engine in ("rev", "whisper", "openai"):
        assert engine in help_text, f"transcribe --engine missing {engine}"
    assert "--lang" in help_text
    assert "--engine" in help_text
    assert "--wor" in help_text
    assert "--nowor" in help_text
    assert "--allow-mps" in help_text


def test_align_exposes_fa_engines():
    help_text = _help("align")
    assert "--engine" in help_text
    for engine in ("wav2vec", "whisper_fa", "qwen"):
        assert engine in help_text, f"align --engine missing {engine}"
    assert "--allow-mps" in help_text


def test_local_inference_device_requires_explicit_mps_opt_in():
    from batchalign.cli._options import inference_device

    assert inference_device(force_cpu=False, allow_mps=False) is None
    assert inference_device(force_cpu=True, allow_mps=False) == "cpu"
    assert inference_device(force_cpu=False, allow_mps=True) == "mps"
    with pytest.raises(typer.BadParameter, match="cannot be used together"):
        inference_device(force_cpu=True, allow_mps=True)


def test_ai_exposes_dspy_engine():
    help_text = _help("ai")
    assert "--engine" in help_text
    assert "dspy" in help_text
    assert "--model" in help_text
    assert "--max-tokens" in help_text
    assert "--timeout" in help_text


def test_ai_passes_instruction_to_inputs(tmp_path, monkeypatch):
    import batchalign as ba

    chat = tmp_path / "sample.cha"
    chat.write_text("@Begin\n@Languages:\teng\n*PAR:\thello .\n@End\n")
    captured = {}

    class FakeBackend:
        pass

    class FakePipeline:
        def run(self, inputs, callbacks=None, outcome_callback=None):
            captured["inputs"] = list(inputs)
            return []

    def fake_recipe(*, ai_backend, **recipe_opts):
        captured["backend"] = ai_backend
        captured["recipe_opts"] = recipe_opts
        return FakePipeline()

    def fake_dspy_backend(**kwargs):
        captured["backend_kwargs"] = kwargs
        return FakeBackend()

    monkeypatch.setattr(ba, "DspyAIBackend", fake_dspy_backend)
    monkeypatch.setattr(ba.recipes, "ai", fake_recipe)

    result = CliRunner().invoke(
        app,
        [
            "--workers",
            "3",
            "ai",
            "revise punctuation",
            str(chat),
            "--out",
            str(tmp_path / "out"),
        ],
    )
    assert result.exit_code == 0, result.output
    inputs = captured["inputs"]
    assert len(inputs) == 1
    assert inputs[0].instruction == "revise punctuation"
    assert str(chat) in str(inputs[0].source_id)
    assert captured["backend_kwargs"]["max_tokens"] == 1024
    assert captured["backend_kwargs"]["timeout"] == 30
    assert captured["recipe_opts"]["workers"] == 3


def test_compare_takes_single_folder_with_gold_template():
    # parity.md: compare reads a template.gold.cha in the input folder, not a
    # parallel gold folder. The CLI should take ONE positional folder.
    help_text = _help("compare")
    assert "template.gold.cha" in help_text
    # The old two-positional (main + gold) form is gone.
    assert "gold reference folder" not in help_text


def test_asr_engine_enum_members():
    from batchalign.cli.transcribe import AsrEngine

    assert {e.value for e in AsrEngine} == {
        "rev", "whisper", "chatwhisper", "openai",
        "funaudio", "tencent", "qwen3", "aliyun", "malayalam", "google",
    }


def test_malayalam_uses_sat_utterance_segmentation(monkeypatch):
    import batchalign as ba
    from batchalign.cli.transcribe import AsrEngine, _build_utseg
    from batchalign.lang import LanguageCode

    sentinel = object()
    monkeypatch.setattr(ba, "MalayalamSaTBackend", lambda: sentinel)

    assert _build_utseg(ba, LanguageCode.from_str("mal"), AsrEngine.malayalam) is sentinel


def test_rev_reuses_raw_word_segmenter_for_typed_pass(monkeypatch):
    import batchalign as ba
    from batchalign.cli.transcribe import AsrEngine, _build_utseg
    from batchalign.lang import LanguageCode

    captured = {}
    shared_segmenter = object()

    def fake_backend(**kwargs):
        captured.update(kwargs)
        return object()

    monkeypatch.setattr(ba, "CHATUtteranceBackend", fake_backend)

    _build_utseg(
        ba,
        LanguageCode.from_str("eng"),
        AsrEngine.rev,
        segmenter=shared_segmenter,
    )

    assert captured["segmenter"] is shared_segmenter


def test_unsupported_utseg_warns_and_retains_asr_utterances(caplog):
    import os
    from types import SimpleNamespace

    from batchalign.cli.transcribe import _build_utseg
    from batchalign.lang import LanguageCode

    with caplog.at_level("WARNING", logger="batchalign.cli.transcribe"):
        backend = _build_utseg(SimpleNamespace(), LanguageCode.from_str("srp"))

    assert backend is None
    assert "retaining ASR-provided utterance boundaries" in caplog.text
    if os.environ.get("BATCHALIGN_PARITY_TRACE"):
        print(f"[utseg-unsupported] backend={backend!r} warning={caplog.text.strip()}")


def test_transcribe_wires_pyannote_ai_for_separate_diarization(
    tmp_path, monkeypatch,
):
    import batchalign as ba

    captured = {}
    speaker = object()
    utterance_segmenter = object()

    monkeypatch.setattr(ba, "WhisperBackend", lambda **_kwargs: object())
    monkeypatch.setattr(
        ba,
        "PyannoteAIBackend",
        lambda **kwargs: captured.update(pyannote_kwargs=kwargs) or speaker,
    )
    monkeypatch.setattr(
        ba,
        "CHATUtteranceBackend",
        lambda **_kwargs: utterance_segmenter,
    )

    class FakePipeline:
        def run(self, _inputs, callbacks=None, outcome_callback=None):
            return []

    def fake_recipe(**kwargs):
        captured["recipe"] = kwargs
        return FakePipeline()

    monkeypatch.setattr(ba.recipes, "transcribe", fake_recipe)
    media = tmp_path / "sample.mp3"
    media.write_bytes(b"media")

    result = CliRunner().invoke(
        app,
        [
            "transcribe",
            "--engine",
            "whisper",
            "--lang",
            "eng",
            "--diarize",
            "--num-speakers",
            "3",
            str(media),
        ],
    )

    assert result.exit_code == 0, result.output
    assert captured["pyannote_kwargs"] == {"num_speakers": 3}
    assert captured["recipe"]["speaker_backend"] is speaker
    assert captured["recipe"]["utseg_backend"] is utterance_segmenter


# ---------------------------------------------------------------------------
# `transcribe --lang` is required and ISO-639-3 only.
# ---------------------------------------------------------------------------


def test_transcribe_language_is_required(tmp_path):
    """No `--lang` → typer rejects before any backend is constructed."""
    import re
    media = tmp_path / "a.wav"
    media.write_bytes(b"")  # path needs to exist for typer.Argument(exists=True)
    runner = CliRunner()
    result = runner.invoke(app, ["transcribe", "--engine", "rev", str(media)])
    assert result.exit_code != 0
    out = (result.output or "") + (result.stderr or "")
    # Rich splits "--lang" across ANSI escapes; strip them first.
    out = re.sub(r"\x1b\[[0-9;]*m", "", out)
    assert "--lang" in out


def test_transcribe_rejects_alpha_2_language(tmp_path, monkeypatch):
    """`--lang en` → BadParameter naming ISO-639-3."""
    media = tmp_path / "a.wav"
    media.write_bytes(b"")
    runner = CliRunner()
    result = runner.invoke(app, [
        "transcribe", "--engine", "rev", "--lang", "en", str(media),
    ])
    assert result.exit_code != 0
    out = (result.output or "") + (result.stderr or "")
    assert "ISO-639-3" in out


def test_transcribe_rejects_auto_language(tmp_path):
    """`--lang auto` is gone; users must pick a real code."""
    media = tmp_path / "a.wav"
    media.write_bytes(b"")
    runner = CliRunner()
    result = runner.invoke(app, [
        "transcribe", "--engine", "rev", "--lang", "auto", str(media),
    ])
    assert result.exit_code != 0


def test_transcribe_accepts_alpha_3_language_and_passes_LanguageCode(
    tmp_path, monkeypatch,
):
    """`--lang eng --engine rev` → RevAI receives a LanguageCode("eng",...).

    Mock RevAI to avoid touching the rev_ai SDK, capture the kwargs.
    """
    import batchalign as ba
    from batchalign.lang import LanguageCode

    captured = {}

    class FakeRevAI:
        # Mirror the real backend's interface enough to satisfy the
        # transcribe pipeline construction path.
        def __init__(self, *, language, num_speakers=2, **kwargs):
            captured["language"] = language
            captured["num_speakers"] = num_speakers
        @property
        def name(self):
            return "fakerev"
        @property
        def batch_policy(self):
            from batchalign.backends.base import BatchPolicy
            return BatchPolicy.one()
        def call(self, batch):
            return []

    monkeypatch.setattr(ba, "RevAI", FakeRevAI)

    # transcribe runs the pipeline; cover that by swapping it for a no-op.
    def fake_recipe(**kwargs):
        class P:
            def run(self, inputs, callbacks=None, **_kwargs):
                return []
        return P()
    monkeypatch.setattr(ba.recipes, "transcribe", fake_recipe)

    media = tmp_path / "a.wav"
    media.write_bytes(b"")

    runner = CliRunner()
    # Patch CHATUtteranceBackend with a no-op too — transcribe ALWAYS
    # segments now (the `--no-segment` flag was removed; the recipe
    # only skips the segmenter when the engine self-segments, which
    # rev does not). Without this stub the test would try to fetch
    # `talkbank/CHATUtterance-en` from HF at runtime.
    class FakeUtseg:
        def __init__(self, *a, **kw): pass
        @property
        def name(self): return "fake-utseg"
        @property
        def batch_policy(self):
            from batchalign.backends.base import BatchPolicy
            return BatchPolicy.one()
        def call(self, batch): return []
    monkeypatch.setattr(ba, "CHATUtteranceBackend", FakeUtseg)

    result = runner.invoke(app, [
        "transcribe", "--engine", "rev", "--lang", "eng", str(media),
    ])
    # The pipeline returns empty outcomes → exit_code 0, no failures.
    assert result.exit_code == 0, result.output
    assert isinstance(captured.get("language"), LanguageCode)
    assert captured["language"].alpha_3 == "eng"
    assert captured["language"].alpha_2 == "en"
    assert captured["language"].name == "English"


@pytest.mark.parametrize(
    ("timing_option", "expected_strip"),
    [
        ([], True),
        (["--wor"], False),
        (["--nowor"], True),
    ],
)
def test_transcribe_omits_word_timing_unless_requested(
    tmp_path, monkeypatch, timing_option, expected_strip,
):
    """Transcribe matches BA2/BA3: `%wor` is opt-in, not the default."""
    import batchalign as ba
    from batchalign.cli import transcribe as transcribe_cli

    written = []

    class FakeBackend:
        pass

    class FakeOutcome:
        def __init__(self, source_id):
            self.source_id = source_id

        def write(self, _path, *, strip_word_timing=False):
            written.append(strip_word_timing)

    class FakeInterface:
        exit_code = 0

        def __enter__(self):
            return self

        def __exit__(self, *_args):
            return False

        def push(self, _task):
            return None

        def run_pipeline(self, _pipeline, inputs, *, on_outcome):
            outcome = FakeOutcome(str(inputs[0].source_id))
            on_outcome(outcome)
            return iter([outcome])

    monkeypatch.setattr(ba, "RevAI", lambda **_kwargs: FakeBackend())
    monkeypatch.setattr(
        ba, "CHATUtteranceBackend", lambda **_kwargs: FakeBackend()
    )
    monkeypatch.setattr(
        ba.recipes, "transcribe", lambda **_kwargs: object()
    )
    monkeypatch.setattr(
        transcribe_cli.Interface, "open", lambda **_kwargs: FakeInterface()
    )

    media = tmp_path / "sample.wav"
    media.write_bytes(b"")
    out = tmp_path / "out"
    result = CliRunner().invoke(
        app,
        [
            "transcribe",
            "--engine",
            "rev",
            "--lang",
            "eng",
            *timing_option,
            "--out",
            str(out),
            str(media),
        ],
    )

    assert result.exit_code == 0, result.output
    assert written == [expected_strip]
