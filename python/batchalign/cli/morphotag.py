"""`morphotag` command — add `%mor` / `%gra` tiers via Stanza."""

from __future__ import annotations

from pathlib import Path

import typer

from ._common import collect_chat, import_ba, write_outcomes


def register(app: typer.Typer) -> None:
    @app.command()
    def morphotag(
        paths: list[Path] = typer.Argument(..., exists=True, help="CHAT files or directories."),
        out: Path = typer.Option(..., "--out", "-o", help="Output directory."),
        language: str = typer.Option("en", "--language", help="Stanza language code."),
        retokenize: bool = typer.Option(False, "--retokenize/--no-retokenize"),
    ) -> None:
        """Add `%mor` and `%gra` tiers via Stanza."""
        ba = import_ba()
        pipeline = ba.recipes.morphotag(
            stanza_backend=ba.StanzaBackend(lang=language, retokenize=retokenize),
        )
        outcomes = pipeline.run(collect_chat(paths))
        write_outcomes(outcomes, out)
