"""Smoke tests for `batchalign3 cache {path,stats,clear}`."""

from __future__ import annotations

from pathlib import Path

from typer.testing import CliRunner

from batchalign.cli import app


runner = CliRunner()


def test_cache_path() -> None:
    result = runner.invoke(app, ["cache", "path"])
    assert result.exit_code == 0
    # default_cache_path lands under dirs.cache_dir() or "." fallback;
    # both produce a non-empty string ending in "batchaligncache.redb".
    assert "batchaligncache.redb" in result.stdout


def test_cache_stats_absent(monkeypatch, tmp_path: Path) -> None:
    fake = tmp_path / "missing.redb"
    monkeypatch.setattr(
        "batchalign.cli.cache._cache_path", lambda: str(fake),
    )
    result = runner.invoke(app, ["cache", "stats"])
    assert result.exit_code == 0
    assert "cache absent" in result.stdout


def test_cache_stats_present(monkeypatch, tmp_path: Path) -> None:
    fake = tmp_path / "present.redb"
    fake.write_bytes(b"x" * 12345)
    monkeypatch.setattr(
        "batchalign.cli.cache._cache_path", lambda: str(fake),
    )
    result = runner.invoke(app, ["cache", "stats"])
    assert result.exit_code == 0
    assert "size:" in result.stdout
    assert "mtime:" in result.stdout
