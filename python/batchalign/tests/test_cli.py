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
