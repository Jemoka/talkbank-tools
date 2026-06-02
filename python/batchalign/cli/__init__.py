"""Typer-based CLI for batchalign.

Each subcommand lives in its own module under `batchalign.cli.<name>`
and registers itself by exposing a `register(app)` function. Adding a
new command is a one-file change:

  1. Create `batchalign/cli/<name>.py` with `register(app)`
  2. Add the entry to `_LAZY_SUBCOMMANDS` below with a static one-line
     help string.

The script is exposed as `batchalign3` via `[project.scripts]` in
`pyproject.toml`.

Subcommand modules are NOT imported at CLI startup. They are loaded on
first dispatch through `LazyTyperGroup` below — so `batchalign3 --help`,
`batchalign3 --version`, and shell-completion of top-level command
names complete without paying the import cost of every backend.
The static help strings let the top-level `--help` panel render the
full command list without loading anything.

The TUI presentation layer lives under `batchalign.cli.tui` —
`Interface` + `Task` are the registerable components every command
uses. The global Typer callback below configures logging (library
silencing happens BEFORE the subcommand body imports any backend)
and stashes the resolved verbosity / plain flag onto `ctx.obj` so
commands can read them when constructing an `Interface`.
"""

from __future__ import annotations

import importlib
from typing import Any

import click
import typer
from typer.core import TyperGroup

from . import _logging
from ._options import CLIOptions, cli_options


# Subcommand name → (module path, one-line help). Help strings are
# duplicated from the subcommand modules' docstrings so the top-level
# `--help` panel can render without loading them. Keep in sync.
_LAZY_SUBCOMMANDS: dict[str, tuple[str, str]] = {
    "transcribe": (
        "batchalign.cli.transcribe",
        "Transcribe media into CHAT (.cha) files.",
    ),
    "align": (
        "batchalign.cli.align",
        "Run forced alignment on existing CHAT files (adds a `%wor` tier).",
    ),
    "morphotag": (
        "batchalign.cli.morphotag",
        "Add `%mor` and `%gra` tiers via Stanza.",
    ),
    "translate": (
        "batchalign.cli.translate",
        "Translate utterances; emits CHAT with `%eng:` tiers.",
    ),
    "utseg": (
        "batchalign.cli.utseg",
        "Utterance segmentation pass over CHAT.",
    ),
    "compare": (
        "batchalign.cli.compare",
        "Compare each transcript in FOLDER against its gold template.",
    ),
    "version": (
        "batchalign.cli.version",
        "Print version, git SHA, and contributors.",
    ),
    "cache": (
        "batchalign.cli.cache",
        "Cache management.",
    ),
    "daemon": (
        "batchalign.cli.daemon",
        "Start the batchalign HTTP daemon in production mode.",
    ),
}


class LazyTyperGroup(TyperGroup):
    """Click group that defers subcommand module imports until dispatch.

    `list_commands` returns the static subcommand list so shell
    completion of top-level names works without loading any module.
    `format_commands` (used by `--help`) uses the static help strings
    in `_LAZY_SUBCOMMANDS` so the help panel renders without loading.
    `get_command` lazy-loads exactly one module — the one being
    dispatched. Per-subcommand `--help` (e.g. `batchalign3 cache --help`)
    pays for that one module only.
    """

    def list_commands(self, ctx: click.Context) -> list[str]:
        names = set(super().list_commands(ctx)) | set(_LAZY_SUBCOMMANDS)
        return sorted(names)

    def get_command(self, ctx: click.Context, cmd_name: str) -> click.Command | None:
        existing = super().get_command(ctx, cmd_name)
        if existing is not None:
            return existing
        if cmd_name in _LAZY_SUBCOMMANDS:
            return self._lazy_load(cmd_name)
        return None

    def format_commands(
        self,
        ctx: click.Context,
        formatter: click.HelpFormatter,
    ) -> None:
        # Use static help so the top-level --help panel does not force
        # every subcommand module to import. Eagerly-registered commands
        # (none, currently) are also included for safety.
        rows: list[tuple[str, str]] = []
        for name in super().list_commands(ctx):
            cmd = super().get_command(ctx, name)
            if cmd is None or getattr(cmd, "hidden", False):
                continue
            rows.append((name, cmd.get_short_help_str() or ""))
        for name, (_mod, short) in _LAZY_SUBCOMMANDS.items():
            rows.append((name, short))
        rows.sort()
        if rows:
            with formatter.section("Commands"):
                formatter.write_dl(rows)

    def _lazy_load(self, cmd_name: str) -> click.Command | None:
        module_path, _help = _LAZY_SUBCOMMANDS[cmd_name]
        mod = importlib.import_module(module_path)
        temp = typer.Typer()
        mod.register(temp)
        click_obj = typer.main.get_command(temp)
        # `register` either adds a single command (`@app.command()`)
        # or attaches a sub-group (`add_typer`). After conversion, the
        # corresponding entry lives in click_obj.commands.
        sub: Any = None
        if isinstance(click_obj, click.MultiCommand):
            sub = click_obj.commands.get(cmd_name)
            if sub is None and len(click_obj.commands) == 1:
                sub = next(iter(click_obj.commands.values()))
        else:
            sub = click_obj
        if sub is not None:
            self.add_command(sub, name=cmd_name)
        return sub


app = typer.Typer(
    name="batchalign3",
    cls=LazyTyperGroup,
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

    # Quiet mode must stay silent — disable the interactive credential
    # prompts that backends opt into via `config.get_*(interactive=True)`.
    from batchalign import config as _ba_config

    _ba_config.suppress_interactive(quiet)

    resolved_plain: bool | None
    if plain and ansi:
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


__all__ = ["app", "CLIOptions", "cli_options", "LazyTyperGroup"]
