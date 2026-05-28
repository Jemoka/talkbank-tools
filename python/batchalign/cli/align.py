"""`align` command — forced alignment on existing CHAT files.

Engine selection mirrors BA2's `align` (Wav2Vec2 for English, Whisper-family
otherwise). Pick with `--engine`:

* ``wav2vec`` — Wav2Vec2 + CTC Viterbi; the model is chosen per the file's
  ``@Languages`` header (BA2's default FA path).
* ``whisperx`` — WhisperX alignment models.

The previous implementation wired ``WhisperBackend`` as the FA backend, but
that class declares only ``ASR`` (not ``FA``), so the pipeline could never
service ``Task.Fa``. The FA backends here declare ``FA`` correctly.
"""

from __future__ import annotations

from enum import Enum
from pathlib import Path
from typing import Any

import typer

from ._common import collect_chat_inputs, write_outcomes
from ._options import cli_options
from .tui import Interface, Task


class FaEngine(str, Enum):
    """Forced-alignment engine selection."""

    wav2vec = "wav2vec"        # torchaudio MMS_FA (BA2 --wav2vec)
    whisper_fa = "whisper_fa"  # Whisper cross-attention DTW (BA2 --whisper_fa)
    whisperx = "whisperx"      # WhisperX (no BA2 oracle on this box)


def register(app: typer.Typer) -> None:
    @app.command()
    def align(
        ctx: typer.Context,
        folder: Path = typer.Argument(
            ...,
            exists=True,
            help="Folder to walk recursively for CHAT files (single file also accepted).",
        ),
        out: Path | None = typer.Option(
            None, "--out", "-o",
            help="Optional output folder; if omitted, each source file is overwritten in place.",
        ),
        engine: FaEngine = typer.Option(
            FaEngine.wav2vec, "--engine", case_sensitive=False,
            help="Forced-alignment engine: wav2vec | whisperx.",
        ),
        model: str | None = typer.Option(
            None, "--model",
            help="FA model id (engine-specific default if omitted; wav2vec picks per the file language).",
        ),
        force_cpu: bool = typer.Option(
            False, "--force-cpu",
            help="Run the FA model on CPU (BA2's --force-cpu). Needed for whisper_fa "
            "on Apple MPS, where Whisper's bfloat16 attention kernel is unsupported.",
        ),
    ) -> None:
        """Run forced alignment on existing CHAT files (adds a `%wor` tier)."""
        import batchalign as ba

        opts = cli_options(ctx)

        with Interface.open(
            command="align",
            params={"engine": engine.value, "fa": model or "(auto)"},
            output=out,
            verbosity=opts.verbosity,
            plain=opts.plain,
            quiet=opts.quiet,
        ) as ui:
            device = "cpu" if force_cpu else None
            fa_backend: Any
            if engine is FaEngine.wav2vec:
                # Language (hence model) comes from each file's @Languages header.
                fa_backend = ba.Wav2Vec2FaBackend(model=model, device=device)
            elif engine is FaEngine.whisper_fa:
                # Whisper cross-attention DTW aligner (BA2 --whisper_fa).
                fa_backend = ba.WhisperFaBackend(model=model, device=device)
            else:
                fa_backend = ba.WhisperXFaBackend(model=model or "large-v2")
            pipeline = ba.recipes.align(fa_backend=fa_backend)
            inputs, root = collect_chat_inputs(folder)
            for inp in inputs:
                ui.push(Task.from_input(inp))
            outcomes = list(ui.run_pipeline(pipeline, inputs))
            write_outcomes(outcomes, root, out)

        raise typer.Exit(code=ui.exit_code)
