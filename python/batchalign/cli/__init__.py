"""Typer-based CLI for batchalign.

Each subcommand lives in its own module under `batchalign.cli.<name>`
and registers itself by exposing a `register(app)` function. Adding a
new command is a one-file change: create `batchalign/cli/<name>.py`
with `register(app)` and append the module to `_COMMAND_MODULES`.

The script is exposed as `batchalign3` via `[project.scripts]` in
`pyproject.toml`.

The TUI presentation layer lives under `batchalign.cli.tui` —
`Interface` + `Task` are the registerable components every command
uses. The global Typer callback below configures logging (library
silencing happens BEFORE the subcommand body imports any backend)
and stashes the resolved verbosity / plain flag onto `ctx.obj` so
commands can read them when constructing an `Interface`.
"""

from __future__ import annotations

import typer

from . import _logging
from ._options import CLIOptions, cli_options
from . import align, compare, daemon, morphotag, transcribe, translate, utseg

app = typer.Typer(
    name="batchalign3",
    no_args_is_help=True,
    add_completion=False,
    help="Batchalign: TalkBank CHAT processing pipeline.",
)


@app.callback()
def _global(
    ctx: typer.Context,
    verbose: int = typer.Option(
        0, "--verbose", "-v",
        count=True,
        help="Increase output verbosity (repeat: -v, -vv, -vvv).",
    ),
    quiet: bool = typer.Option(
        False, "--quiet", "-q",
        help="Suppress all output except errors and the final summary.",
    ),
    plain: bool = typer.Option(
        False, "--plain",
        help="Force the column-aligned non-live renderer (default: auto-detect).",
    ),
    ansi: bool = typer.Option(
        False, "--ansi",
        help="Force the live renderer even when stdout is not a TTY.",
    ),
) -> None:
    """Global flags. Runs before any subcommand body."""
    verbosity = -1 if quiet else verbose
    _logging.configure(verbosity)

    resolved_plain: bool | None
    if plain and ansi:
        # Explicit conflict — prefer plain (the more conservative).
        resolved_plain = True
    elif plain:
        resolved_plain = True
    elif ansi:
        resolved_plain = False
    else:
        resolved_plain = None  # auto-detect inside Interface.open

    ctx.obj = CLIOptions(
        verbosity=verbosity,
        plain=resolved_plain,
        quiet=quiet,
    )


_COMMAND_MODULES = [
    transcribe,
    align,
    morphotag,
    translate,
    utseg,
    compare,
    daemon,  # HTTP daemon for the desktop GUI; ships via the [api] extra
    # coref,
    # opensmile,
    # avqi,
]

for _mod in _COMMAND_MODULES:
    _mod.register(app)


__all__ = ["app", "CLIOptions", "cli_options"]
