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
    # post-LMDB migration the path is a directory named `cache.lmdb`.
    assert "cache.lmdb" in result.stdout


def test_cache_stats_absent(monkeypatch, tmp_path: Path) -> None:
    fake = tmp_path / "missing.lmdb"
    monkeypatch.setattr(
        "batchalign.cli.cache._cache_path", lambda: str(fake),
    )
    result = runner.invoke(app, ["cache", "stats"])
    assert result.exit_code == 0
    assert "cache absent" in result.stdout


def test_cache_stats_present_dir(monkeypatch, tmp_path: Path) -> None:
    # LMDB layout: a directory containing data.mdb + lock.mdb.
    fake = tmp_path / "present.lmdb"
    fake.mkdir()
    (fake / "data.mdb").write_bytes(b"x" * 12345)
    (fake / "lock.mdb").write_bytes(b"y" * 1000)
    monkeypatch.setattr(
        "batchalign.cli.cache._cache_path", lambda: str(fake),
    )
    result = runner.invoke(app, ["cache", "stats"])
    assert result.exit_code == 0
    assert "size:" in result.stdout
    assert "mtime:" in result.stdout
    # The size should reflect the sum of the inner files (~13 KiB).
    assert "(13345 bytes)" in result.stdout


def test_cache_stats_present_legacy_file(monkeypatch, tmp_path: Path) -> None:
    # Legacy redb single-file layout — still reported cleanly.
    fake = tmp_path / "present.redb"
    fake.write_bytes(b"x" * 12345)
    monkeypatch.setattr(
        "batchalign.cli.cache._cache_path", lambda: str(fake),
    )
    result = runner.invoke(app, ["cache", "stats"])
    assert result.exit_code == 0
    assert "size:" in result.stdout
    assert "mtime:" in result.stdout
