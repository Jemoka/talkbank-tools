"""`morphotag` command — add `%mor` / `%gra` tiers via Stanza."""

from __future__ import annotations

from pathlib import Path

import typer

from ._common import collect_chat_inputs, write_outcomes
from ._options import cli_options
from .tui import Interface, Task


def register(app: typer.Typer) -> None:
    @app.command()
    def morphotag(
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
        language: str = typer.Option("en", "--language", help="Stanza language code."),
        retokenize: bool = typer.Option(False, "--retokenize/--no-retokenize"),
    ) -> None:
        """Add `%mor` and `%gra` tiers via Stanza."""
        import batchalign as ba

        opts = cli_options(ctx)

        with Interface.open(
            command="morphotag",
            params={"lang": language, "retokenize": retokenize},
            output=out,
            verbosity=opts.verbosity,
            plain=opts.plain,
            quiet=opts.quiet,
        ) as ui:
            pipeline = ba.recipes.morphotag(
                stanza_backend=ba.StanzaBackend(lang=language, retokenize=retokenize),
            )
            inputs, root = collect_chat_inputs(folder)
            for inp in inputs:
                ui.push(Task.from_input(inp))
            outcomes = list(ui.run_pipeline(pipeline, inputs))
            write_outcomes(outcomes, root, out)

        raise typer.Exit(code=ui.exit_code)
