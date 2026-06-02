"""`batchalign3 version` command — print version, git SHA, contributors.

Banner art is the ported BA2 figlet; `__version__` comes from the
installed `batchalign` package metadata; the git SHA is read in this
order:

1. `BATCHALIGN_GIT_SHA` env var — explicit override for CI / wheel
   builds where the runtime SHA must differ from the baked-in one.
2. `batchalign._core.BATCHALIGN_GIT_SHA` — baked into the Rust
   extension at build time (via `bazel/stamp.sh` →
   `rustc_env = {"VERGEN_GIT_SHA": "{STABLE_GIT_HASH}"}` on the engine
   `rust_library`, or via the engine/core `build.rs` on the Cargo
   path). This is the canonical source for installed wheels — same
   SHA the Rust runtime stamps into ASR-generated CHAT files.
3. `git rev-parse --short HEAD` against the source tree (`.git` walk)
   — last-resort fallback for editable installs in a checkout where
   the extension hasn't been (re)built yet.
4. `unknown` if nothing resolves.
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
    "Houjun Liu",
    "Franklin Chen",
    "Brian MacWhinney",
)


def _resolve_version() -> str:
    try:
        return metadata.version("batchalign")
    except metadata.PackageNotFoundError:
        return "unknown"


def _resolve_git_sha() -> str:
    # 1. Explicit env override (CI / wheel-build overrides).
    env = os.environ.get("BATCHALIGN_GIT_SHA")
    if env:
        return env.strip()

    # 2. The Rust extension is the canonical source for installed
    #    wheels. Bazel's `--workspace_status_command=bazel/stamp.sh`
    #    stamps `VERGEN_GIT_SHA` onto the `rust_library` /
    #    `rust_shared_library` at build time; the Cargo path goes
    #    through `build.rs`. Either way the value is re-exported as
    #    `_core.BATCHALIGN_GIT_SHA`.
    try:
        from batchalign import _core  # type: ignore[attr-defined]

        sha = getattr(_core, "BATCHALIGN_GIT_SHA", None)
        if sha and sha != "unknown":
            return sha.strip()
    except ImportError:
        # Extension not built yet — fall through to .git walk.
        pass

    # 3. Editable install in a source checkout — walk up to a `.git`.
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
        f"  TalkBank | batchalign3 {version}  (git {sha})",
        "",
        "  Maintainers:",
        *[f"    - {m}" for m in _MAINTAINERS],
        "",
        "  talkbank.org",
        "",
    ]
    return "\n".join(lines)


def register(app: typer.Typer) -> None:
    @app.command()
    def version() -> None:
        """Print version, git SHA, and contributors."""
        typer.echo(render())
