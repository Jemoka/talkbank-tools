"""Resolved global CLI flags.

Lives in its own module (rather than on `cli/__init__.py`) so that
command modules — which `cli/__init__.py` imports during package
load — can pull these types without triggering a circular import.
"""

from __future__ import annotations

from dataclasses import dataclass

import typer


@dataclass
class CLIOptions:
    """Resolved global flags. Stashed on the Typer context."""
    verbosity: int = 0          # -q → -1; -v counts; -vv = 2; ...
    plain: bool | None = None   # None → auto-detect from TTY
    quiet: bool = False


def cli_options(ctx: typer.Context) -> CLIOptions:
    """Read the resolved global flags off the Typer context.

    A small accessor so command files don't reach into `ctx.obj`
    directly. If the callback didn't run (e.g. tests invoking a
    subcommand in isolation), returns defaults.
    """
    obj = ctx.obj
    return obj if isinstance(obj, CLIOptions) else CLIOptions()


__all__ = ["CLIOptions", "cli_options"]
