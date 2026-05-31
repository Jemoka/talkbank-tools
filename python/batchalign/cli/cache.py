"""`batchalign3 cache` — inspect and manage the local result cache.

Backed by the redb store at `default_cache_path()` (see
`crates/batchalign/batchalign-engine/src/cache.rs`). Three subcommands:

- `cache path`  — print the resolved cache path.
- `cache stats` — print size on disk and last-mtime.
- `cache clear` — delete the cache file via `nuke_cache()`.

A `prewarm` subcommand is intentionally NOT included yet — it would
require loading every wired backend just to ping its model, which is
heavier than this CLI surface should pull in. Track it as a follow-up
once the Stanza cache (Landing 2) ships and we can prewarm cheaply.
"""

from __future__ import annotations

import os
import sys
from datetime import datetime, timezone
from pathlib import Path

import typer

app = typer.Typer(help="Inspect and manage the batchalign result cache.")


def _default_cache_path_fallback() -> Path:
    """Pure-Python mirror of cache.rs::default_cache_path()."""
    if sys.platform == "darwin":
        base = Path.home() / "Library" / "Caches"
    elif sys.platform.startswith("linux"):
        base = Path(os.environ.get("XDG_CACHE_HOME", str(Path.home() / ".cache")))
    elif sys.platform.startswith("win"):
        base = Path(os.environ.get("LOCALAPPDATA", str(Path.home() / "AppData" / "Local")))
    else:
        base = Path(".")
    return base / "batchalign" / "batchaligncache.redb"


def _cache_path() -> str:
    try:
        from batchalign import default_cache_path  # type: ignore[attr-defined]

        return str(default_cache_path())
    except (ImportError, AttributeError):
        # Falls back when the PyO3 extension isn't built — keeps `cache
        # path` / `stats` usable without maturin develop.
        return str(_default_cache_path_fallback())


@app.command("path")
def cache_path() -> None:
    """Print the resolved cache path."""
    typer.echo(_cache_path())


@app.command("stats")
def cache_stats() -> None:
    """Print cache size on disk and last-mtime."""
    path = _cache_path()
    if not os.path.exists(path):
        typer.echo(f"cache absent: {path}")
        raise typer.Exit(code=0)
    st = os.stat(path)
    mtime = datetime.fromtimestamp(st.st_mtime, tz=timezone.utc).isoformat()
    size_mb = st.st_size / (1024 * 1024)
    typer.echo(f"path:  {path}")
    typer.echo(f"size:  {size_mb:.2f} MiB ({st.st_size} bytes)")
    typer.echo(f"mtime: {mtime}")


@app.command("clear")
def cache_clear(
    yes: bool = typer.Option(
        False, "--yes", "-y", help="Skip the confirmation prompt.",
    ),
) -> None:
    """Delete the cache file."""
    path = _cache_path()
    if not yes:
        typer.confirm(f"Delete {path}?", abort=True)
    try:
        from batchalign import nuke_cache  # type: ignore[attr-defined]

        nuke_cache()
    except (ImportError, AttributeError):
        # PyO3 extension not built — fall back to direct file removal.
        try:
            os.remove(path)
        except FileNotFoundError:
            pass
    typer.echo(f"cleared {path}")


def register(parent: typer.Typer) -> None:
    parent.add_typer(app, name="cache", help="Cache management.")
