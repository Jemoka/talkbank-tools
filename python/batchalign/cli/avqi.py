"""`avqi` command — Acoustic Voice Quality Index (stub)."""

from __future__ import annotations

from pathlib import Path

import typer


def register(app: typer.Typer) -> None:
    @app.command()
    def avqi(
        paths: list[Path] = typer.Argument(..., exists=True, help="Media files or directories."),
        out: Path = typer.Option(..., "--out", "-o", help="Output directory."),
    ) -> None:
        """Extract Acoustic Voice Quality Index."""
        raise typer.BadParameter(
            "No backend shipped for this task yet. "
            "Use the Python API once `AvqiBackend` is available.",
        )
