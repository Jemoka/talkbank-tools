"""`align` command — forced alignment on existing CHAT files."""

from __future__ import annotations

from pathlib import Path

import typer

from ._common import collect_chat, import_ba, write_outcomes


def register(app: typer.Typer) -> None:
    @app.command()
    def align(
        paths: list[Path] = typer.Argument(..., exists=True, help="CHAT files or directories."),
        out: Path = typer.Option(..., "--out", "-o", help="Output directory."),
        model: str = typer.Option("openai/whisper-large-v3", "--model"),
    ) -> None:
        """Run forced alignment on existing CHAT files."""
        ba = import_ba()
        pipeline = ba.recipes.align(fa_backend=ba.WhisperBackend(model=model))
        outcomes = pipeline.run(collect_chat(paths))
        write_outcomes(outcomes, out)
