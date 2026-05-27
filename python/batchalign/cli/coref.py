"""`coref` command — coreference resolution (stub, no backend bundled)."""

from __future__ import annotations

from pathlib import Path

import typer
from rich.console import Console


def register(app: typer.Typer) -> None:
    @app.command()
    def coref(
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
    ) -> None:
        """Coreference resolution (stub — wire a real coref backend)."""
        c = Console()
        c.print("[red]fail[/]  coref: no backend shipped with this build")
        c.print("      [dim]hint:[/] use the Python API: "
                "`ba.recipes.coref(coref_backend=...)`")
        raise typer.Exit(code=2)
