"""`avqi` command — Acoustic Voice Quality Index (stub)."""

from __future__ import annotations

from pathlib import Path

import typer
from rich.console import Console


def register(app: typer.Typer) -> None:
    @app.command()
    def avqi(
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
    ) -> None:
        """Extract Acoustic Voice Quality Index."""
        c = Console()
        c.print("[red]fail[/]  avqi: no backend shipped with this build")
        c.print("      [dim]hint:[/] use the Python API once `AvqiBackend` is available")
        raise typer.Exit(code=2)
