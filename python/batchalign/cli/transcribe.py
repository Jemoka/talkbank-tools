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
        num_speakers: int = typer.Option(0, "--num-speakers", help="Expected speakers (0 = unknown)."),
        fa: bool = typer.Option(True, "--fa/--no-fa", help="Run forced alignment."),
        diarize: bool = typer.Option(False, "--diarize/--no-diarize", help="Run speaker diarization."),
    ) -> None:
        """Transcribe media into CHAT (.cha) files."""
        ba = import_ba()
        asr = ba.WhisperBackend(model=model)
        fa_backend = ba.WhisperBackend(model=model) if fa else None
        speaker = ba.PyannoteBackend() if diarize else None
        pipeline = ba.recipes.transcribe(
            asr_backend=asr,
            fa_backend=fa_backend,
            speaker_backend=speaker,
            language=language,
            num_speakers=num_speakers,
        )
        outcomes = pipeline.run(collect_media(paths))
        write_outcomes(outcomes, out)
