"""`opensmile` command — OpenSMILE acoustic features (stub)."""

from __future__ import annotations

from pathlib import Path

import typer
from rich.console import Console


def register(app: typer.Typer) -> None:
    @app.command()
    def opensmile(
        folder: Path = typer.Argument(
            ...,
            exists=True,
            help="Folder to walk recursively for media files (single file also accepted).",
        ),
        out: Path | None = typer.Option(
            None,
            "--out",
            "-o",
            help="Optional output folder; if omitted, features are written next to each source media file.",
        ),
        feature_set: str = typer.Option("eGeMAPSv02", "--feature-set"),
    ) -> None:
        """Extract OpenSMILE acoustic features."""
        c = Console()
        c.print("[red]fail[/]  opensmile: no backend shipped with this build")
        c.print("      [dim]hint:[/] use the Python API once `OpenSmileBackend` is available")
        raise typer.Exit(code=2)
