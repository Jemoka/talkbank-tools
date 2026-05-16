"""Typer-based CLI for batchalign.

Each subcommand lives in its own module under `batchalign.cli.<name>`
and registers itself by exposing a `register(app)` function. Adding a
new command is a one-file change: create `batchalign/cli/<name>.py`
with `register(app)` and append the module to `_COMMAND_MODULES`.

The script is exposed as `batchalign3` via `[project.scripts]` in
`pyproject.toml`.
"""

from __future__ import annotations

import typer

from . import align, avqi, compare, coref, morphotag, opensmile, transcribe, translate, utseg

app = typer.Typer(
    name="batchalign3",
    no_args_is_help=True,
    add_completion=False,
    help="Batchalign: TalkBank CHAT processing pipeline.",
)

_COMMAND_MODULES = [
    transcribe,
    align,
    morphotag,
    translate,
    utseg,
    compare,
    coref,
    opensmile,
    avqi,
]

for _mod in _COMMAND_MODULES:
    _mod.register(app)


__all__ = ["app"]
