"""`transcribe` command — media files into CHAT transcripts.

Engine selection mirrors BA2's `transcribe` (which chose between Rev.AI and
the Whisper family): pick the ASR engine with `--engine`. All BA2 engines are
reachable — Rev.AI, WhisperX, HF Whisper, OpenAI Whisper, and a vLLM-served
Whisper (preferred for local GPU/Metal boxes). Language is propagated to the
chosen backend (the kernel ships `Auto`, so the backend's `language=` pin is
what actually reaches the model).
"""

from __future__ import annotations

from enum import Enum
from pathlib import Path
from typing import Any

import typer

from ._common import collect_media_inputs, write_outcomes
from ._options import cli_options
from .tui import Interface, Task


class AsrEngine(str, Enum):
    """ASR engine selection (BA2 parity set + vLLM)."""

    rev = "rev"            # Rev.AI cloud (ASR + its own diarization)
    whisperx = "whisperx"  # WhisperX (faster-whisper + alignment)
    whisper = "whisper"    # HF transformers Whisper
    chatwhisper = "chatwhisper"  # TalkBank CHATWhisper + BERT utterance segmenter (BA2 default)
    openai = "openai"      # openai-whisper package
    vllm = "vllm"          # vLLM-served Whisper via the OpenAI audio API


# Sensible per-engine default models (BA2's defaults where they exist).
_DEFAULT_MODEL = {
    AsrEngine.whisperx: "large-v2",
    AsrEngine.whisper: "openai/whisper-large-v3",
    AsrEngine.openai: "turbo",
    AsrEngine.vllm: "openai/whisper-large-v3",
}


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
            None, "--out", "-o",
            help="Optional output folder; if omitted, each `.cha` transcript is written next to its source media.",
        ),
        engine: AsrEngine = typer.Option(
            AsrEngine.whisperx, "--engine", case_sensitive=False,
            help="ASR engine: rev | whisperx | whisper | openai | vllm.",
        ),
        language: str = typer.Option("auto", "--language", help="Language code or 'auto'."),
        model: str | None = typer.Option(None, "--model", help="ASR model id (engine-specific default if omitted)."),
        diarize: bool = typer.Option(False, "--diarize/--no-diarize", help="Run speaker diarization (ignored for rev, which diarizes itself)."),
        num_speakers: int = typer.Option(2, "--num-speakers", "-n", help="Expected speaker count (diarization hint)."),
        vllm_url: str = typer.Option("http://localhost:8000/v1", "--vllm-url", help="Base URL for the vLLM OpenAI-compatible server (engine=vllm)."),
        segment: bool = typer.Option(
            True, "--segment/--no-segment",
            help="Run BA2's CHATUtterance BERT utterance segmentation after ASR "
            "(ignored for chatwhisper, which segments internally).",
        ),
    ) -> None:
        """Transcribe media into CHAT (.cha) files.

        Forced alignment is *not* part of `transcribe` — run `batchalign align`
        afterwards for refined word-level timings.
        """
        import batchalign as ba

        opts = cli_options(ctx)

        with Interface.open(
            command="transcribe",
            params={"engine": engine.value, "asr": model or _DEFAULT_MODEL.get(engine, ""), "lang": language, "diarize": diarize},
            output=out,
            verbosity=opts.verbosity,
            plain=opts.plain,
            quiet=opts.quiet,
        ) as ui:
            asr_backend, rev_diarizes = _build_asr(ba, engine, model, language, vllm_url)
            # Rev does its own diarization; for the Whisper family add Pyannote
            # when --diarize is requested.
            speaker_backend: Any = None
            if diarize and not rev_diarizes:
                speaker_backend = ba.PyannoteBackend()
            # BA2 pairing: ASR → CHATUtterance BERT segmentation. chatwhisper
            # segments internally; others get the segmenter when a model exists
            # for the language.
            utseg_backend: Any = None
            if segment and engine is not AsrEngine.chatwhisper:
                utseg_backend = _build_utseg(ba, language)
            pipeline = ba.recipes.transcribe(
                asr_backend=asr_backend,
                speaker_backend=speaker_backend,
                utseg_backend=utseg_backend,
            )
            inputs, root = collect_media_inputs(folder)
            for inp in inputs:
                ui.push(Task.from_input(inp))
            outcomes = list(ui.run_pipeline(pipeline, inputs))
            write_outcomes(outcomes, root, out, output_suffix=".cha")

        raise typer.Exit(code=ui.exit_code)


def _build_asr(ba: Any, engine: AsrEngine, model: str | None, language: str, vllm_url: str):
    """Construct the ASR backend for `engine`. Returns (backend, rev_diarizes)."""
    m = model or _DEFAULT_MODEL.get(engine)
    if engine is AsrEngine.rev:
        return ba.RevAI(language=language), True
    if engine is AsrEngine.whisperx:
        return ba.WhisperXBackend(model=m, language=None if language == "auto" else language), False
    if engine is AsrEngine.whisper:
        return ba.WhisperBackend(model=m, language=language), False
    if engine is AsrEngine.chatwhisper:
        # CHATWhisper resolves its own model per language; it also performs
        # BA2's BERT utterance segmentation internally (one segment per
        # utterance), so it needs no separate UtSeg stage.
        return ba.ChatWhisperBackend(lang="eng" if language in ("auto", "en") else language), False
    if engine is AsrEngine.openai:
        return ba.OpenAIWhisperBackend(model=m, language=language), False
    if engine is AsrEngine.vllm:
        return ba.VllmAsrBackend(model=m, language=language, base_url=vllm_url), False
    raise typer.BadParameter(f"unknown engine: {engine}")


# CLI language → CHATUtterance resolve key (BA2 only ships en/zh/yue models).
_UTSEG_LANG = {
    "auto": "eng", "en": "eng", "eng": "eng",
    "zh": "zho", "zho": "zho", "zh-hans": "zho", "cmn": "zho",
    "yue": "yue",
}


def _build_utseg(ba: Any, language: str):
    """Build the CHATUtterance segmenter for `language`, or None if BA2 ships
    no utterance model for it (then ASR segments stand as utterances)."""
    key = _UTSEG_LANG.get(language.lower())
    if key is None:
        return None
    try:
        return ba.CHATUtteranceBackend(lang=key)
    except ValueError:
        return None
