"""`transcribe` command — media files into CHAT transcripts.

Engine selection mirrors BA2's `transcribe` (which chose between Rev.AI and
the Whisper family): pick the ASR engine with `--engine`. All BA2 engines are
reachable — Rev.AI, WhisperX, HF Whisper, OpenAI Whisper, and a vLLM-served
Whisper (preferred for local GPU/Metal boxes). Language is propagated to the
chosen backend (the kernel ships `Auto`, so the backend's `language=` pin is
what actually reaches the model).

Language convention: users always type an ISO-639-3 alpha_3 code
(`eng`, `cmn`, `yue`, `spa`, …). The CLI validates eagerly via
`batchalign.lang.LanguageCode`; each backend receives the resolved
record and reads whichever form its vendor SDK needs.
"""

from __future__ import annotations

from enum import Enum
from pathlib import Path
from typing import Any

import typer

from ..lang import LanguageCode
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
    funaudio = "funaudio"  # FunASR SenseVoiceSmall / paraformer-zh (BA2 FunAudioEngine)
    tencent = "tencent"    # Tencent Cloud ASR (BA2 TencentEngine)
    qwen3 = "qwen3"        # Qwen3-ASR (Alibaba open-weight; tbtbt parity)


# Sensible per-engine default models (BA2's defaults where they exist).
_DEFAULT_MODEL = {
    AsrEngine.whisperx: "large-v2",
    AsrEngine.whisper: "openai/whisper-large-v3",
    AsrEngine.openai: "turbo",
    AsrEngine.vllm: "openai/whisper-large-v3",
    AsrEngine.funaudio: "FunAudioLLM/SenseVoiceSmall",
    AsrEngine.qwen3: "Qwen/Qwen3-ASR-1.7B",
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
            AsrEngine.rev, "--engine", case_sensitive=False,
            help="ASR engine: rev | whisperx | whisper | openai | vllm.",
        ),
        lang: str = typer.Option(
            ...,
            "--lang",
            help="ISO-639-3 alpha_3 code: eng, cmn, yue, spa, … (Required.)",
        ),
        model: str | None = typer.Option(None, "--model", help="ASR model id (engine-specific default if omitted)."),
        diarize: bool = typer.Option(False, "--diarize/--no-diarize", help="Run speaker diarization (ignored for rev, which diarizes itself)."),
        num_speakers: int = typer.Option(2, "--num-speakers", "-n", help="Expected speaker count (diarization hint)."),
        vllm_url: str = typer.Option("http://localhost:8000/v1", "--vllm-url", help="Base URL for the vLLM OpenAI-compatible server (engine=vllm)."),
        segment: bool = typer.Option(
            True, "--segment/--no-segment",
            help="Run BA2's CHATUtterance BERT utterance segmentation after ASR.",
        ),
        force_cpu: bool = typer.Option(
            False, "--force-cpu",
            help="Run the ASR model on CPU (BA2's --force-cpu). Needed on Apple "
            "MPS, where Whisper's bfloat16 attention kernel is unsupported.",
        ),
    ) -> None:
        """Transcribe media into CHAT (.cha) files.

        Forced alignment is *not* part of `transcribe` — run `batchalign align`
        afterwards for refined word-level timings.
        """
        import batchalign as ba

        opts = cli_options(ctx)

        # Validate the language string at the CLI boundary so failures
        # surface before the heavy backend constructors run.
        try:
            lang_code = LanguageCode.from_str(lang)
        except ValueError as exc:
            raise typer.BadParameter(str(exc), param_hint="--lang") from exc

        with Interface.open(
            command="transcribe",
            params={"engine": engine.value, "asr": model or _DEFAULT_MODEL.get(engine, ""), "lang": lang_code.alpha_3, "diarize": diarize},
            output=out,
            verbosity=opts.verbosity,
            plain=opts.plain,
            quiet=opts.quiet,
        ) as ui:
            device = "cpu" if force_cpu else None
            asr_backend, rev_diarizes = _build_asr(
                ba, engine, model, lang_code, vllm_url, num_speakers, device
            )
            # Rev does its own diarization; for the Whisper family add Pyannote
            # when --diarize is requested.
            speaker_backend: Any = None
            if diarize and not rev_diarizes:
                speaker_backend = ba.PyannoteBackend()
            # BA2 pairing: ASR → CHATUtterance BERT segmentation + disfluency/
            # retrace cleanup, applied uniformly to every ASR engine's word
            # stream (rev, chatwhisper, …) when a segmenter model exists.
            utseg_backend: Any = None
            if segment:
                utseg_backend = _build_utseg(ba, lang_code, engine)
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


def _build_asr(
    ba: Any,
    engine: AsrEngine,
    model: str | None,
    lang: LanguageCode,
    vllm_url: str,
    num_speakers: int = 2,
    device: str | None = None,
):
    """Construct the ASR backend for `engine`. Returns (backend, rev_diarizes).

    Every backend takes the resolved `LanguageCode` directly. Each
    one's constructor pulls whichever form (`alpha_2`, `alpha_3`, or
    `name`) its underlying SDK expects.
    """
    m = model or _DEFAULT_MODEL.get(engine)
    if engine is AsrEngine.rev:
        return ba.RevAI(language=lang, num_speakers=num_speakers), True
    if engine is AsrEngine.whisperx:
        return ba.WhisperXBackend(model=m, language=lang, device=device), False
    if engine is AsrEngine.whisper:
        return ba.WhisperBackend(model=m, language=lang, device=device), False
    if engine is AsrEngine.chatwhisper:
        return ba.ChatWhisperBackend(language=lang, device=device), False
    if engine is AsrEngine.openai:
        return ba.OpenAIWhisperBackend(model=m, language=lang, device=device), False
    if engine is AsrEngine.vllm:
        return ba.VllmAsrBackend(model=m, language=lang, base_url=vllm_url), False
    if engine is AsrEngine.funaudio:
        return ba.FunAudioBackend(
            model=m or "FunAudioLLM/SenseVoiceSmall",
            language=lang,
            device=device,
        ), False
    if engine is AsrEngine.tencent:
        # Tencent Cloud does its own diarization; the UtSeg pairing still applies.
        return ba.TencentAsrBackend(language=lang, num_speakers=num_speakers), True
    if engine is AsrEngine.qwen3:
        # Qwen3-ASR (Alibaba); single-speaker output (no diarization). tbtbt
        # pins device default to CPU on Apple Silicon (MPS degraded on 1.7B);
        # --force-cpu honors that explicitly.
        return ba.Qwen3AsrBackend(
            language=lang, model_id=m or "Qwen/Qwen3-ASR-1.7B",
            device=device or "cpu",
        ), False
    raise typer.BadParameter(f"unknown engine: {engine}")


# CHATUtterance only ships segmenter models for English, Chinese, and
# Cantonese — keyed by ISO-639-3.
_UTSEG_LANG_3 = {
    "eng": "eng",
    "cmn": "zho",   # Mandarin → Chinese segmenter
    "zho": "zho",
    "yue": "yue",
}


def _build_utseg(ba: Any, lang: LanguageCode, engine: AsrEngine | None = None):
    """Build the CHATUtterance segmenter for `lang`, or None if BA2 ships
    no utterance model for it (then ASR segments stand as utterances).

    BA2's FunAudioEngine always segments with `BertCantoneseUtteranceModel`
    — even when the ASR model is `funasr/paraformer-zh` (Mandarin). Mirror
    that quirk so paraformer-zh BA3 output is byte-identical to BA2's.
    """
    key = _UTSEG_LANG_3.get(lang.alpha_3)
    if key is None:
        return None
    try:
        cantonese = engine is AsrEngine.funaudio
        return ba.CHATUtteranceBackend(lang=key, cantonese_inference=cantonese)
    except ValueError:
        return None
