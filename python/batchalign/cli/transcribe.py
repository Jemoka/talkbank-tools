"""`transcribe` command — media files into CHAT transcripts."""

from __future__ import annotations

from pathlib import Path

import typer

from ._common import collect_media_inputs, write_outcomes
from ._options import cli_options
from .tui import Interface, Task


def register(app: typer.Typer) -> None:
    @app.command()
    def transcribe(
        ctx: typer.Context,
        folder: Path = typer.Argument(
            ...,
            exists=True,
            help="Folder to walk recursively for media files (single file also accepted).",
        ),
        out: Path | None = typer.Option(
            None,
            "--out",
            "-o",
            help="Optional output folder; if omitted, each `.cha` transcript is written next to its source media file.",
        ),
        language: str = typer.Option("auto", "--language", help="Language code or 'auto'."),
        model: str = typer.Option("openai/whisper-large-v3", "--model", help="ASR model id."),
        diarize: bool = typer.Option(False, "--diarize/--no-diarize", help="Run speaker diarization."),
    ) -> None:
        """Transcribe media into CHAT (.cha) files.

        Forced alignment is *not* part of `transcribe` — compose
        `batchalign align` (or `ba.recipes.align(...)`) afterwards if you
        want refined word-level timings.
        """
        import batchalign as ba

        opts = cli_options(ctx)

        # Interface.open prints the header + spinner immediately. The
        # backend constructors (which load ML models and can take many
        # seconds) run INSIDE the `with` block so the user sees the
        # "preparing pipeline…" indicator during that time.
        with Interface.open(
            command="transcribe",
            params={"asr": model, "diarize": diarize, "lang": language},
            output=out,
            verbosity=opts.verbosity,
            plain=opts.plain,
            quiet=opts.quiet,
        ) as ui:
            pipeline = ba.recipes.transcribe(
                asr_backend=ba.WhisperBackend(model=model),
                speaker_backend=ba.PyannoteBackend() if diarize else None,
            )
            inputs, root = collect_media_inputs(folder)
            for inp in inputs:
                ui.push(Task.from_input(inp))
            outcomes = list(ui.run_pipeline(pipeline, inputs))
            write_outcomes(outcomes, root, out, output_suffix=".cha")

        raise typer.Exit(code=ui.exit_code)
