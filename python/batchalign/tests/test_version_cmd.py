"""Smoke tests for the `batchalign3 version` command."""

from __future__ import annotations

import os

from typer.testing import CliRunner

from batchalign.cli import app
from batchalign.cli import version as version_mod


runner = CliRunner()


def test_version_command_runs() -> None:
    result = runner.invoke(app, ["version"])
    assert result.exit_code == 0
    assert "batchalign3 v" in result.stdout
    assert "Maintainers:" in result.stdout


def test_version_resolves_sha_from_env(monkeypatch) -> None:
    monkeypatch.setenv("BATCHALIGN_GIT_SHA", "abc1234")
    assert version_mod._resolve_git_sha() == "abc1234"


def test_version_falls_back_to_unknown(monkeypatch, tmp_path) -> None:
    monkeypatch.delenv("BATCHALIGN_GIT_SHA", raising=False)
    # Point _resolve_git_sha at an isolated path with no .git anywhere
    # on the parent chain. We do this by patching __file__ on the module
    # to a tempfile under tmp_path.
    fake_file = tmp_path / "version.py"
    fake_file.touch()
    monkeypatch.setattr(version_mod, "__file__", str(fake_file))
    assert version_mod._resolve_git_sha() == "unknown"


def test_version_render_contains_version() -> None:
    text = version_mod.render()
    assert "batchalign3 v" in text
    assert "git " in text
