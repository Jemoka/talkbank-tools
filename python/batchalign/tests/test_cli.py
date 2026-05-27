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
    "coref",
    "utseg",
    "compare",
    "opensmile",
    "avqi",
    "daemon",
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
    # Collapse the Rich box-drawing wrapping so option/value tokens that get
    # split across lines are still greppable as substrings.
    out = CliRunner().invoke(app, [cmd, "--help"]).output
    return "".join(ch for ch in out if ch not in "│─╭╮╰╯").replace("\n", " ")


def test_transcribe_exposes_all_ba2_asr_engines():
    # parity.md: every engine BA2 supported must be reachable. The transcribe
    # CLI must offer rev / whisperx / whisper / openai, plus the vLLM path.
    help_text = _help("transcribe")
    for engine in ("rev", "whisperx", "whisper", "openai", "vllm"):
        assert engine in help_text, f"transcribe --engine missing {engine}"
    assert "--language" in help_text
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

    assert {e.value for e in AsrEngine} == {"rev", "whisperx", "whisper", "openai", "vllm"}
