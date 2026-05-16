"""`coref` command — coreference resolution (stub, no backend bundled)."""

from __future__ import annotations

from pathlib import Path

import typer


def register(app: typer.Typer) -> None:
    @app.command()
    def coref(
        paths: list[Path] = typer.Argument(..., exists=True, help="CHAT files or directories."),
        out: Path = typer.Option(..., "--out", "-o", help="Output directory."),
    ) -> None:
        """Coreference resolution (stub — wire a real coref backend)."""
        raise typer.BadParameter(
            "No backend shipped for this task yet. "
            "Use the Python API: `ba.recipes.coref(coref_backend=...)`.",
        )
