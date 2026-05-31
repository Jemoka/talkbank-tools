"""`batchalign3 version` command — print version, git SHA, contributors.

Banner art is the ported BA2 figlet; `__version__` comes from the
installed `batchalign` package metadata; the git SHA is read in this
order:

1. `BATCHALIGN_GIT_SHA` env var (CI bakes this into release wheels).
2. `git rev-parse --short HEAD` if the source tree has a `.git`.
3. `unknown` if neither is available.

Once the Rust binary embeds `VERGEN_GIT_SHA` via a `build.rs` (see the
Landing 7 plan), this command will additionally exec the binary with
`--print-sha` to surface the Rust-side SHA and warn on mismatch with
the Python-side. For now the Python side is the source of truth.
"""

from __future__ import annotations

import os
import subprocess
from importlib import metadata
from pathlib import Path

import typer


_BANNER = r"""
 ____        _       _           _ _
| __ )  __ _| |_ ___| |__   __ _| (_) __ _ _ __
|  _ \ / _` | __/ __| '_ \ / _` | | |/ _` | '_ \
| |_) | (_| | || (__| | | | (_| | | | (_| | | | |
|____/ \__,_|\__\___|_| |_|\__,_|_|_|\__, |_| |_|
                                     |___/
"""

_MAINTAINERS = (
    "Houjun Liu (houjun@jemoka.com)",
    "Brian MacWhinney (TalkBank)",
)


def _resolve_version() -> str:
    try:
        return metadata.version("batchalign")
    except metadata.PackageNotFoundError:
        return "unknown"


def _resolve_git_sha() -> str:
    env = os.environ.get("BATCHALIGN_GIT_SHA")
    if env:
        return env.strip()
    # Walk up from this file looking for a .git directory; if found, ask
    # git for the short SHA. Subprocess errors fall through to "unknown".
    here = Path(__file__).resolve()
    for parent in (here, *here.parents):
        if (parent / ".git").exists():
            try:
                out = subprocess.check_output(
                    ["git", "rev-parse", "--short", "HEAD"],
                    cwd=parent,
                    stderr=subprocess.DEVNULL,
                    text=True,
                )
                return out.strip()
            except (subprocess.CalledProcessError, FileNotFoundError):
                return "unknown"
    return "unknown"


def render() -> str:
    """Return the banner string. Pure for testability."""
    version = _resolve_version()
    sha = _resolve_git_sha()
    lines = [
        _BANNER,
        f"  batchalign3 v{version}  (git {sha})",
        "",
        "  Maintainers:",
        *[f"    - {m}" for m in _MAINTAINERS],
        "",
        "  Source: https://github.com/Jemoka/talkbank-tools",
        "",
    ]
    return "\n".join(lines)


def register(app: typer.Typer) -> None:
    @app.command()
    def version() -> None:
        """Print version, git SHA, and contributors."""
        typer.echo(render())
