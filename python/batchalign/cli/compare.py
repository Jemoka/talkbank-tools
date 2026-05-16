"""`compare` command — compare a main transcript against a gold reference."""

from __future__ import annotations

from pathlib import Path

import typer

from ._common import import_ba, write_outcomes


def register(app: typer.Typer) -> None:
    @app.command()
    def compare(
        main: Path = typer.Argument(..., exists=True, help="Main transcript (CHAT file)."),
        gold: Path = typer.Argument(..., exists=True, help="Gold reference (CHAT file)."),
        out: Path = typer.Option(..., "--out", "-o", help="Output directory."),
    ) -> None:
        """Compare a main transcript against a gold reference (pure AST)."""
        ba = import_ba()
        from batchalign.inputs import paired_from_paths

        pipeline = ba.recipes.compare()
        outcomes = pipeline.run([paired_from_paths(str(main), str(gold))])
        write_outcomes(outcomes, out)
