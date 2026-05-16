"""Shared helpers for the batchalign Typer CLI.

Centralizes:
* Input collection — expand positional paths (files or directories) into
  the typed `BAValue` objects pipelines consume.
* Output writing — ensure the output directory exists and dispatch each
  outcome's own `.write()`.
* Config-key resolution with a clear error message naming the INI key
  and env var when a credential is missing.
"""

from __future__ import annotations

from pathlib import Path
from typing import Any, Iterable

import typer

from batchalign import config as _ba_config


def collect_media(paths: Iterable[Path]) -> list[Any]:
    """Expand positional `paths` into `BAValue` media inputs."""
    from batchalign.inputs import iter_media, media_from_path

    items: list[Any] = []
    for p in paths:
        if p.is_dir():
            items.extend(iter_media(p))
        else:
            items.append(media_from_path(p))
    return items


def collect_chat(paths: Iterable[Path]) -> list[Any]:
    """Expand positional `paths` into `BAValue` CHAT inputs."""
    from batchalign.inputs import iter_chat, chat_from_path

    items: list[Any] = []
    for p in paths:
        if p.is_dir():
            items.extend(iter_chat(p))
        else:
            items.append(chat_from_path(p))
    return items


def write_outcomes(outcomes: list[Any], out_dir: Path) -> None:
    """Create `out_dir` and dispatch `.write()` on each outcome."""
    out_dir.mkdir(parents=True, exist_ok=True)
    for outcome in outcomes:
        sid = getattr(outcome, "source_id", None) or "output"
        outcome.write(str(out_dir / f"{sid}.cha"))


def import_ba() -> Any:
    """Lazy import of the top-level `batchalign` package.

    Defers loading `_core` so `--help` still works on a fresh clone
    without a compiled .so.
    """
    import batchalign as ba

    return ba


def require_api_key(provider: str, ini_key: str, env_var: str) -> str:
    """Return the configured key or raise a Typer-friendly error.

    The error message names the exact INI key and env var so users can
    immediately fix the configuration.
    """
    key = _ba_config.get_api_key(provider)
    if key:
        return key
    raise typer.BadParameter(
        f"No {provider} key. Set {env_var} env var or "
        f"`{ini_key}` in ~/.batchalign.ini",
    )


__all__ = [
    "collect_media",
    "collect_chat",
    "write_outcomes",
    "import_ba",
    "require_api_key",
]
