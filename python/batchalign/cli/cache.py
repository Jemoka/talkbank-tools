"""`batchalign3 cache` — inspect and manage the local result cache.

Backed by the LMDB store at `default_cache_path()` (see
`crates/batchalign/batchalign-engine/src/cache.rs`). The cache is a
DIRECTORY (`cache.lmdb/`) containing `data.mdb` + `lock.mdb`, not a
single file like the old redb backend. `stats` walks the directory;
`clear` removes it recursively via the Rust `nuke_cache()` (which
also knows how to clean up legacy redb single-file installs).

Subcommands:

- `cache path`  — print the resolved cache path.
- `cache stats` — print total size on disk and last-mtime.
- `cache clear` — delete the cache via `nuke_cache()`.

A `prewarm` subcommand is intentionally NOT included yet — it would
require loading every wired backend just to ping its model, which is
heavier than this CLI surface should pull in. Track it as a follow-up
once the Stanza cache (Landing 2) ships and we can prewarm cheaply.
"""

from __future__ import annotations

import os
import shutil
import sys
from datetime import datetime, timezone
from pathlib import Path

import typer

app = typer.Typer(help="Inspect and manage the batchalign result cache.")


def _default_cache_path_fallback() -> Path:
    """Pure-Python mirror of cache.rs::default_cache_path().

    Used only when the PyO3 extension is unavailable (fresh checkout,
    no `maturin develop` / `bazel build` yet). Must stay in sync with
    the Rust implementation; the Rust path is authoritative.
    """
    if sys.platform == "darwin":
        base = Path.home() / "Library" / "Caches"
    elif sys.platform.startswith("linux"):
        base = Path(os.environ.get("XDG_CACHE_HOME", str(Path.home() / ".cache")))
    elif sys.platform.startswith("win"):
        base = Path(os.environ.get("LOCALAPPDATA", str(Path.home() / "AppData" / "Local")))
    else:
        base = Path(".")
    return base / "batchalign" / "cache.lmdb"


def _cache_path() -> str:
    try:
        from batchalign import default_cache_path  # type: ignore[attr-defined]

        return str(default_cache_path())
    except (ImportError, AttributeError):
        # Falls back when the PyO3 extension isn't built — keeps `cache
        # path` / `stats` usable without maturin develop / bazel build.
        return str(_default_cache_path_fallback())


def _dir_size_bytes(root: Path) -> int:
    """Total size of all files under ``root`` (recursive). LMDB's
    `data.mdb` is sparse on Linux/macOS, so this reports apparent
    size, which is what users typically care about."""
    total = 0
    for dirpath, _dirs, files in os.walk(root):
        for f in files:
            try:
                total += os.stat(os.path.join(dirpath, f)).st_size
            except OSError:
                # File vanished between walk and stat (concurrent
                # writer). Skip; the total is approximate anyway.
                pass
    return total


def _latest_mtime(root: Path) -> float:
    """Newest mtime across the cache directory's contents. Falls back
    to the directory's own mtime if it's empty."""
    newest = root.stat().st_mtime
    for dirpath, _dirs, files in os.walk(root):
        for f in files:
            try:
                m = os.stat(os.path.join(dirpath, f)).st_mtime
                if m > newest:
                    newest = m
            except OSError:
                pass
    return newest


@app.command("path")
def cache_path() -> None:
    """Print the resolved cache path."""
    typer.echo(_cache_path())


@app.command("stats")
def cache_stats() -> None:
    """Print cache size on disk and last-mtime.

    Handles both the current LMDB layout (a directory) and the legacy
    redb layout (a single file) so old caches still report cleanly
    until they're cleared.
    """
    path_str = _cache_path()
    path = Path(path_str)
    if not path.exists():
        typer.echo(f"cache absent: {path_str}")
        raise typer.Exit(code=0)
    if path.is_dir():
        size = _dir_size_bytes(path)
        mtime = _latest_mtime(path)
    else:
        # Legacy single-file (redb) install.
        st = path.stat()
        size = st.st_size
        mtime = st.st_mtime
    mtime_iso = datetime.fromtimestamp(mtime, tz=timezone.utc).isoformat()
    size_mb = size / (1024 * 1024)
    typer.echo(f"path:  {path_str}")
    typer.echo(f"size:  {size_mb:.2f} MiB ({size} bytes)")
    typer.echo(f"mtime: {mtime_iso}")


@app.command("clear")
def cache_clear(
    yes: bool = typer.Option(
        False, "--yes", "-y", help="Skip the confirmation prompt.",
    ),
) -> None:
    """Delete the cache directory (or legacy file)."""
    path_str = _cache_path()
    path = Path(path_str)
    if not yes:
        typer.confirm(f"Delete {path_str}?", abort=True)
    try:
        from batchalign import nuke_cache  # type: ignore[attr-defined]

        # Rust nuke_cache handles both directory (LMDB) and file
        # (legacy redb) layouts.
        nuke_cache()
    except (ImportError, AttributeError):
        # PyO3 extension not built — best-effort fallback covering
        # both layouts.
        if path.is_dir():
            shutil.rmtree(path, ignore_errors=True)
        else:
            try:
                os.remove(path)
            except FileNotFoundError:
                pass
    typer.echo(f"cleared {path_str}")


def register(parent: typer.Typer) -> None:
    parent.add_typer(app, name="cache", help="Cache management.")
