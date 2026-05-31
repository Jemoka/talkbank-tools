"""Smoke-test the Typer CLI: every subcommand's `--help` must succeed."""

from __future__ import annotations

import pytest
from typer.testing import CliRunner

from batchalign.cli import app


SUBCOMMANDS = [
    "transcribe",
    "align",
    "morphotag",
    "translate",
    "utseg",
    "compare",
]


def test_root_help():
    runner = CliRunner()
    result = runner.invoke(app, ["--help"])
    assert result.exit_code == 0
    for cmd in SUBCOMMANDS:
        assert cmd in result.output, f"missing {cmd} in root --help"


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
    # parity.md: every engine BA2 supported must be reachable. The transcribe
    # CLI must offer rev / whisperx / whisper / openai, plus the vLLM path.
    help_text = _help("transcribe")
    for engine in ("rev", "whisperx", "whisper", "openai", "vllm"):
        assert engine in help_text, f"transcribe --engine missing {engine}"
    assert "--lang" in help_text
    assert "--engine" in help_text


def test_align_exposes_fa_engines():
    help_text = _help("align")
    assert "--engine" in help_text
    for engine in ("wav2vec", "whisperx"):
        assert engine in help_text, f"align --engine missing {engine}"


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
        "rev", "whisperx", "whisper", "chatwhisper", "openai", "vllm",
        "funaudio", "tencent", "qwen3",
    }


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
            def run(self, inputs, callbacks):
                return []
        return P()
    monkeypatch.setattr(ba.recipes, "transcribe", fake_recipe)

    media = tmp_path / "a.wav"
    media.write_bytes(b"")

    runner = CliRunner()
    result = runner.invoke(app, [
        # `--no-segment` keeps CHATUtteranceBackend (which would fetch
        # talkbank/CHATUtterance-en at runtime) out of the path so the
        # test runs hermetically without network/HF cache permissions.
        "transcribe", "--engine", "rev", "--lang", "eng",
        "--no-segment", str(media),
    ])
    # The pipeline returns empty outcomes → exit_code 0, no failures.
    assert result.exit_code == 0, result.output
    assert isinstance(captured.get("language"), LanguageCode)
    assert captured["language"].alpha_3 == "eng"
    assert captured["language"].alpha_2 == "en"
    assert captured["language"].name == "English"
