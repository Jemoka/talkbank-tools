"""Standalone speaker diarization over existing timed CHAT transcripts."""

from __future__ import annotations

from enum import Enum
from pathlib import Path
from typing import Any

import typer

from ._common import collect_chat_inputs, write_outcome
from ._options import cli_options
from .tui import Interface, Task


class DiarizeEngine(str, Enum):
    """Speaker diarization backend exposed by the CLIs."""

    pyannote_ai = "pyannote-ai"
    pyannote = "pyannote"


def _build_backend(ba: Any, engine: DiarizeEngine, num_speakers: int) -> Any:
    if engine is DiarizeEngine.pyannote_ai:
        return ba.PyannoteAIBackend(num_speakers=num_speakers)
    return ba.PyannoteBackend(num_speakers=num_speakers)


def register(app: typer.Typer) -> None:
    @app.command()
    def diarize(
        ctx: typer.Context,
        folder: Path = typer.Argument(
            ...,
            exists=True,
            help="Timed CHAT file or folder to scan recursively; matching media is resolved from @Media or the transcript stem.",
        ),
        out: Path | None = typer.Option(
            None,
            "--out",
            "-o",
            help="Optional output folder; if omitted, each source CHAT file is overwritten in place.",
        ),
        engine: DiarizeEngine = typer.Option(
            DiarizeEngine.pyannote_ai,
            "--engine",
            case_sensitive=False,
            help="Diarization engine: pyannote-ai (cloud) or pyannote (local).",
        ),
        num_speakers: int = typer.Option(
            0,
            "--num-speakers",
            "-n",
            min=0,
            help="Expected speaker count; zero auto-detects.",
        ),
    ) -> None:
        """Diarize timed CHAT and write speaker assignments back into CHAT."""
        import batchalign as ba

        opts = cli_options(ctx)

        with Interface.open(
            command="diarize",
            params={
                "engine": engine.value,
                "num_speakers": num_speakers or "auto",
            },
            output=out,
            verbosity=opts.verbosity,
            plain=opts.plain,
            quiet=opts.quiet,
        ) as ui:
            pipeline = ba.recipes.diarize(
                speaker_backend=_build_backend(ba, engine, num_speakers),
                workers=opts.workers,
            )
            inputs, root = collect_chat_inputs(folder)
            for inp in inputs:
                ui.push(Task.from_input(inp))
            list(
                ui.run_pipeline(
                    pipeline,
                    inputs,
                    on_outcome=lambda outcome: write_outcome(
                        outcome, root, out
                    ),
                )
            )

        raise typer.Exit(code=ui.exit_code)


__all__ = ["DiarizeEngine", "_build_backend", "register"]
