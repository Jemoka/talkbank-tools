"""`utseg` command — utterance segmentation over CHAT files."""

from __future__ import annotations

from pathlib import Path

import typer

from ._common import collect_chat_inputs, write_outcomes
from ._options import cli_options
from .tui import Interface, Task


def register(app: typer.Typer) -> None:
    @app.command()
    def utseg(
        ctx: typer.Context,
        folder: Path = typer.Argument(
            ...,
            exists=True,
            help="Folder to walk recursively for CHAT files (single file also accepted).",
        ),
        out: Path | None = typer.Option(
            None,
            "--out",
            "-o",
            help="Optional output folder; if omitted, each source file is overwritten in place.",
        ),
        stanza_fallback: bool = typer.Option(False, "--stanza-fallback/--no-stanza-fallback"),
        language: str = typer.Option("en", "--language"),
    ) -> None:
        """Utterance segmentation pass over CHAT."""
        import batchalign as ba

        opts = cli_options(ctx)

        with Interface.open(
            command="utseg",
            params={"lang": language, "stanza_fallback": stanza_fallback},
            output=out,
            verbosity=opts.verbosity,
            plain=opts.plain,
            quiet=opts.quiet,
        ) as ui:
            pipeline = ba.recipes.utseg(
                utseg_backend=ba.PyannoteBackend(),
                stanza_fallback=stanza_fallback,
            )
            inputs, root = collect_chat_inputs(folder)
            for inp in inputs:
                ui.push(Task.from_input(inp))
            outcomes = list(ui.run_pipeline(pipeline, inputs))
            write_outcomes(outcomes, root, out)

        raise typer.Exit(code=ui.exit_code)
