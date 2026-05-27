"""`translate` command — emits CHAT with `%eng:` translation tiers."""

from __future__ import annotations

from enum import Enum
from pathlib import Path
from typing import Any

import typer

from ._common import collect_chat_inputs, write_outcomes
from ._options import cli_options
from .tui import Interface, Task


class TranslateEngine(str, Enum):
    """Translation backend selection."""

    google = "google"
    vllm = "vllm"


def register(app: typer.Typer) -> None:
    @app.command()
    def translate(
        ctx: typer.Context,
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
        target: str = typer.Option("eng", "--target", help="Target language code (ISO 639-3)."),
        engine: TranslateEngine = typer.Option(
            TranslateEngine.google,
            "--engine",
            case_sensitive=False,
        ),
    ) -> None:
        """Translate utterances; emits CHAT with `%eng:` tiers."""
        import batchalign as ba

        opts = cli_options(ctx)

        with Interface.open(
            command="translate",
            params={"engine": engine.value, "target": target},
            output=out,
            verbosity=opts.verbosity,
            plain=opts.plain,
            quiet=opts.quiet,
        ) as ui:
            backend: Any
            if engine is TranslateEngine.google:
                backend = ba.GoogleTranslateBackend(target=target)
            else:
                backend = ba.VllmTranslateBackend(target=target)
            pipeline = ba.recipes.translate(translate_backend=backend)
            inputs, root = collect_chat_inputs(folder)
            for inp in inputs:
                ui.push(Task.from_input(inp))
            outcomes = list(ui.run_pipeline(pipeline, inputs))
            write_outcomes(outcomes, root, out)

        raise typer.Exit(code=ui.exit_code)
