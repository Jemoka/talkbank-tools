"""Regression coverage for fatal morphotag setup failures."""

from __future__ import annotations

from typer.testing import CliRunner

from batchalign.cli import app


def test_morphotag_setup_failure_exits_nonzero(tmp_path, monkeypatch):
    """A broken Stanza install must not be reported as a successful run."""
    import batchalign as ba

    chat = tmp_path / "sample.cha"
    chat.write_text("@Begin\n@Languages:\teng\n*PAR:\thello .\n@End\n")

    def fail_backend(**_kwargs):
        raise RuntimeError("torch runtime is incomplete")

    monkeypatch.setattr(ba, "StanzaBackend", fail_backend)
    result = CliRunner().invoke(app, ["--plain", "morphotag", str(chat)])

    assert result.exit_code == 2
    assert "pipeline aborted" in result.output
    assert "torch runtime is incomplete" in result.output
