"""`opensmile` command — OpenSMILE acoustic features (stub)."""

from __future__ import annotations

from pathlib import Path

import typer


def register(app: typer.Typer) -> None:
    @app.command()
    def opensmile(
        paths: list[Path] = typer.Argument(..., exists=True, help="Media files or directories."),
        out: Path = typer.Option(..., "--out", "-o", help="Output directory."),
        feature_set: str = typer.Option("eGeMAPSv02", "--feature-set"),
    ) -> None:
        """Extract OpenSMILE acoustic features."""
        raise typer.BadParameter(
            "No backend shipped for this task yet. "
            "Use the Python API once `OpenSmileBackend` is available.",
        )
