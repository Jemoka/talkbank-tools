"""`transcribe` command — media files into CHAT transcripts."""

from __future__ import annotations

from pathlib import Path

import typer

from ._common import collect_media, import_ba, write_outcomes


def register(app: typer.Typer) -> None:
    @app.command()
    def transcribe(
        paths: list[Path] = typer.Argument(
            ...,
            exists=True,
            help="Media files or directories.",
        ),
        out: Path = typer.Option(
            ...,
            "--out",
            "-o",
            help="Output directory.",
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
        ba = import_ba()
        asr = ba.WhisperBackend(model=model)
        speaker = ba.PyannoteBackend() if diarize else None
        pipeline = ba.recipes.transcribe(
            asr_backend=asr,
            speaker_backend=speaker,
        )
        outcomes = pipeline.run(collect_media(paths))
        write_outcomes(outcomes, out)
