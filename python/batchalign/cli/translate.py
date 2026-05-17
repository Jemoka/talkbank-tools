"""`translate` command — emits CHAT with `%eng:` translation tiers."""

from __future__ import annotations

from enum import Enum
from pathlib import Path
from typing import Any

import typer

from ._common import collect_chat, import_ba, write_outcomes


class TranslateEngine(str, Enum):
    """Translation backend selection."""

    google = "google"
    vllm = "vllm"


def register(app: typer.Typer) -> None:
    @app.command()
    def translate(
        paths: list[Path] = typer.Argument(..., exists=True, help="CHAT files or directories."),
        out: Path = typer.Option(..., "--out", "-o", help="Output directory."),
        target: str = typer.Option("eng", "--target", help="Target language code (ISO 639-3)."),
        engine: TranslateEngine = typer.Option(
            TranslateEngine.google,
            "--engine",
            case_sensitive=False,
        ),
    ) -> None:
        """Translate utterances; emits CHAT with `%eng:` tiers."""
        ba = import_ba()
        backend: Any
        if engine is TranslateEngine.google:
            backend = ba.GoogleTranslateBackend(target=target)
        else:
            backend = ba.VllmTranslateBackend(target=target)
        pipeline = ba.recipes.translate(translate_backend=backend)
        outcomes = pipeline.run(collect_chat(paths))
        write_outcomes(outcomes, out)
