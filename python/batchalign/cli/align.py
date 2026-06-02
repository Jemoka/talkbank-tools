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

from ..lang import LanguageCode
from ._common import collect_chat_inputs, write_outcomes
from ._options import cli_options
from .tui import Interface, Task


def _infer_lang(folder: Path) -> LanguageCode:
    """Read the first CHAT file's `@Languages:` header and return its
    primary language code. Matches the per-file resolution the Rust
    runners do for FA / Morphosyntax / UTR.

    Used to construct the Rev.AI / Whisper UTR backend, which (unlike
    Stanza or Qwen3 FA) requires the language at construction time.
    """
    files: list[Path]
    if folder.is_file():
        files = [folder]
    else:
        files = sorted(folder.rglob("*.cha"))
    for path in files:
        try:
            with open(path, "r", encoding="utf-8") as fh:
                for line in fh:
                    if line.startswith("@Languages:"):
                        value = line.split(":", 1)[1].strip()
                        # First language wins (CHAT lists primary first).
                        primary = value.replace(",", " ").split()[0]
                        return LanguageCode.from_str(primary)
        except OSError:
            continue
    raise typer.BadParameter(
        f"no @Languages: header found in any CHAT under {folder}; "
        "cannot pick a UTR backend language. Pass --utr-engine off to skip UTR."
    )


class FaEngine(str, Enum):
    """Forced-alignment engine selection."""

    wav2vec = "wav2vec"        # torchaudio MMS_FA (BA2 --wav2vec)
    whisper_fa = "whisper_fa"  # Whisper cross-attention DTW (BA2 --whisper_fa)
    qwen = "qwen"              # Qwen3-ForcedAligner-0.6B (BA3 cutover Landing 6 #29)


class UtrEngine(str, Enum):
    """Utterance Timing Recovery backend selection.

    UTR runs an ASR pass over the whole audio and Hirschberg-aligns the
    CHAT word stream to the resulting tokens to recover utterance bullets
    on untimed transcripts. The "engine" here is just an ASR backend that
    has opted into the UTR marker — same backend you'd use for ASR.
    """

    off = "off"
    whisper = "whisper"
    rev = "rev"


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
            help="Forced-alignment engine: wav2vec | whisper_fa | qwen.",
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
        utr_engine: UtrEngine = typer.Option(
            UtrEngine.rev, "--utr-engine", case_sensitive=False,
            help="Utterance Timing Recovery backend: rev | whisper | off. "
            "When non-off, runs `Task.Utr` before FA to recover utterance "
            "bullets on fully-untimed CHATs. Automatically skipped when "
            "*any* utterance already carries a bullet — UTR is intended "
            "for fully-untimed transcripts only.",
        ),
        utr_model: str | None = typer.Option(
            None, "--utr-model",
            help="UTR model id (only used when --utr-engine=whisper; default "
            "is openai/whisper-large-v3 to match BA2's transcribe).",
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
            elif engine is FaEngine.qwen:
                # Qwen3 ForcedAligner standalone (no ASR pass).
                fa_backend = ba.Qwen3FaBackend(
                    model_id=model or "Qwen/Qwen3-ForcedAligner-0.6B",
                    device=device or "cpu",
                )
            else:
                raise typer.BadParameter(f"unknown engine: {engine}")

            utr_backend: Any | None
            if utr_engine is UtrEngine.off:
                utr_backend = None
            else:
                # Rev.AI / Whisper need the language at construction time
                # (unlike Stanza / Qwen3 FA, which read it per-call from
                # the wire input). Infer it from the first CHAT's
                # @Languages: header — same source the Rust runners use.
                lang_code = _infer_lang(folder)
                if utr_engine is UtrEngine.whisper:
                    utr_backend = ba.WhisperBackend(
                        model=utr_model or "openai/whisper-large-v3",
                        language=lang_code,
                        device=device,
                    )
                elif utr_engine is UtrEngine.rev:
                    # Credentials come from the user's batchalign config.
                    utr_backend = ba.RevAI(language=lang_code)
                else:
                    raise typer.BadParameter(f"unknown UTR engine: {utr_engine}")

            pipeline = ba.recipes.align(fa_backend=fa_backend, utr_backend=utr_backend)
            inputs, root = collect_chat_inputs(folder)
            for inp in inputs:
                ui.push(Task.from_input(inp))
            outcomes = list(ui.run_pipeline(pipeline, inputs))
            write_outcomes(outcomes, root, out)

        raise typer.Exit(code=ui.exit_code)
