"""`ai` command - generic AI transcript editing over CHAT files."""

from __future__ import annotations

from enum import Enum
from pathlib import Path

import typer

from ._common import collect_ai_inputs, write_outcome
from ._options import cli_options
from .tui import Interface, Task


class AIEngine(str, Enum):
    """Generic AI backend selection."""

    dspy = "dspy"


def register(app: typer.Typer) -> None:
    @app.command()
    def ai(
        ctx: typer.Context,
        instruction: str = typer.Argument(
            ...,
            help="Instruction applied to every utterance.",
        ),
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
        engine: AIEngine = typer.Option(
            AIEngine.dspy,
            "--engine",
            case_sensitive=False,
        ),
        model: str = typer.Option(
            "zai-org/GLM-5.2",
            "--model",
            help="DSPy LM model string.",
        ),
        max_tokens: int = typer.Option(
            1024,
            "--max-tokens",
            min=1,
            help="Maximum output tokens for the DSPy LM call.",
        ),
        timeout: int = typer.Option(
            30,
            "--timeout",
            min=1,
            help="Per-utterance DSPy LM timeout in seconds.",
        ),
    ) -> None:
        """Run generic AI transcript editing."""
        import batchalign as ba

        opts = cli_options(ctx)

        with Interface.open(
            command="ai",
            params={"engine": engine.value, "model": model},
            output=out,
            verbosity=opts.verbosity,
            plain=opts.plain,
            quiet=opts.quiet,
        ) as ui:
            if engine is AIEngine.dspy:
                backend = ba.DspyAIBackend(
                    model="openai/"+model,
                    max_tokens=max_tokens,
                    timeout=timeout,
                )
            else:
                raise typer.BadParameter(f"unknown engine: {engine}")
            pipeline = ba.recipes.ai(ai_backend=backend, workers=opts.workers)
            inputs, root = collect_ai_inputs(folder, instruction=instruction)
            for inp in inputs:
                ui.push(Task.from_input(inp))
            list(
                ui.run_pipeline(
                    pipeline,
                    inputs,
                    on_outcome=lambda outcome: write_outcome(outcome, root, out),
                )
            )

        raise typer.Exit(code=ui.exit_code)
