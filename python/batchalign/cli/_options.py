"""Resolved global CLI flags.

Lives in its own module (rather than on `cli/__init__.py`) so that
command modules — which `cli/__init__.py` imports during package
load — can pull these types without triggering a circular import.
"""

from __future__ import annotations

import logging
from dataclasses import dataclass

import typer


@dataclass
class CLIOptions:
    """Resolved global flags. Stashed on the Typer context."""
    verbosity: int = 0          # -q → -1; -v counts; -vv = 2; ...
    plain: bool | None = None   # None → auto-detect from TTY
    quiet: bool = False
    workers: int = 8


def cli_options(ctx: typer.Context) -> CLIOptions:
    """Read the resolved global flags off the Typer context.

    A small accessor so command files don't reach into `ctx.obj`
    directly. If the callback didn't run (e.g. tests invoking a
    subcommand in isolation), returns defaults.
    """
    obj = ctx.obj
    return obj if isinstance(obj, CLIOptions) else CLIOptions()


def inference_device(*, force_cpu: bool, allow_mps: bool) -> str | None:
    """Resolve mutually exclusive CLI device switches for local ML backends.

    CPU remains the safe default when a backend does not select CUDA itself.
    Apple MPS is therefore explicit: callers must request it, and model
    loaders receive the concrete ``"mps"`` selector so the choice is not
    dependent on ambient Torch defaults.
    """
    if force_cpu and allow_mps:
        raise typer.BadParameter(
            "--force-cpu and --allow-mps cannot be used together",
            param_hint="--allow-mps",
        )
    if force_cpu:
        return "cpu"
    if allow_mps:
        logging.getLogger(__name__).warning(
            "--allow-mps: using the Apple GPU for local inference; sustained "
            "MPS workloads can trigger rare driver stalls. Models that "
            "select a dtype explicitly remain on float32."
        )
        return "mps"
    return None


__all__ = ["CLIOptions", "cli_options", "inference_device"]
