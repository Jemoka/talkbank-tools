"""`utseg` command — utterance segmentation over CHAT files."""

from __future__ import annotations

from pathlib import Path

import typer

from ._common import collect_chat, import_ba, write_outcomes


def register(app: typer.Typer) -> None:
    @app.command()
    def utseg(
        paths: list[Path] = typer.Argument(..., exists=True, help="CHAT files or directories."),
        out: Path = typer.Option(..., "--out", "-o", help="Output directory."),
        stanza_fallback: bool = typer.Option(False, "--stanza-fallback/--no-stanza-fallback"),
        language: str = typer.Option("en", "--language"),
    ) -> None:
        """Utterance segmentation pass over CHAT."""
        ba = import_ba()
        pipeline = ba.recipes.utseg(
            utseg_backend=ba.PyannoteBackend(),
            stanza_fallback=stanza_fallback,
        )
        outcomes = pipeline.run(collect_chat(paths))
        write_outcomes(outcomes, out)
