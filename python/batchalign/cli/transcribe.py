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
from ._common import collect_media_inputs, write_outcome
from ._options import cli_options, inference_device
from .tui import Interface, Task


class AsrEngine(str, Enum):
    """ASR engine selection (BA2 parity set)."""

    rev = "rev"            # Rev.AI cloud (ASR + its own diarization)
    whisper = "whisper"    # HF transformers Whisper
    chatwhisper = "chatwhisper"  # TalkBank CHATWhisper + BERT utterance segmenter (BA2 default)
    openai = "openai"      # openai-whisper package
    funaudio = "funaudio"  # FunASR SenseVoiceSmall / paraformer-zh (BA2 FunAudioEngine)
    tencent = "tencent"    # Tencent Cloud ASR (BA2 TencentEngine)
    qwen3 = "qwen3"        # Qwen3-ASR (Alibaba open-weight; tbtbt parity)
    aliyun = "aliyun"      # Aliyun NLS Cloud ASR (BA2 AlibabaEngine)
    malayalam = "malayalam"  # gvs Wav2Vec2 XLSR Malayalam CTC
    google = "google"      # Gemini audio ASR + native speaker diarization


# Engines that ship internal sentence segmentation — skip CHATUtterance
# BERT pairing for these. Currently nothing qualifies: ChatWhisperBackend
# strips punctuation and emits one raw blob per file, deliberately leaving
# segmentation to the downstream UtSeg stage (matches BA2's pairing). Kept
# as a predicate so adding a self-segmenting backend later is one line.
def _engine_self_segments(engine: AsrEngine) -> bool:
    _ = engine
    return False


# Sensible per-engine default models (BA2's defaults where they exist).
_DEFAULT_MODEL = {
    AsrEngine.google: "gemini-3.5-flash",
    AsrEngine.whisper: "openai/whisper-large-v3",
    AsrEngine.openai: "turbo",
    AsrEngine.funaudio: "FunAudioLLM/SenseVoiceSmall",
    AsrEngine.qwen3: "Qwen/Qwen3-ASR-1.7B",
    AsrEngine.malayalam: "gvs/wav2vec2-large-xlsr-malayalam",
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
            help="ASR engine: rev | google | whisper | chatwhisper | openai | funaudio | tencent | qwen3 | malayalam.",
        ),
        lang: str = typer.Option(
            ...,
            "--lang",
            help="ISO-639-3 alpha_3 code: eng, cmn, yue, spa, … (Required.)",
        ),
        model: str | None = typer.Option(None, "--model", help="ASR model id (engine-specific default if omitted)."),
        diarize: bool = typer.Option(
            False,
            "--diarize/--no-diarize",
            help="Run speaker diarization (ignored for rev and google, which diarize themselves).",
        ),
        num_speakers: int = typer.Option(2, "--num-speakers", "-n", help="Expected speaker count (diarization hint)."),
        force_cpu: bool = typer.Option(
            False, "--force-cpu",
            help="Run the ASR model on CPU (BA2's --force-cpu). Needed on Apple "
            "MPS, where Whisper's bfloat16 attention kernel is unsupported.",
        ),
        allow_mps: bool = typer.Option(
            False, "--allow-mps",
            help="Explicitly allow local ASR models to use Apple MPS. Off by "
            "default because sustained MPS inference can be unstable; "
            "CHATWhisper remains float32 when selected.",
        ),
        nowor: bool = typer.Option(False, "--nowor", help="Omit word-level timing (`%wor` and inline word bullets)."),
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
            device = inference_device(force_cpu=force_cpu, allow_mps=allow_mps)
            asr_backend, native_diarization = _build_asr(
                ba, engine, model, lang_code, num_speakers, device
            )
            # Cloud engines with native speaker labels share one atomic call;
            # other engines opt into Pyannote with --diarize.
            speaker_backend: Any = None
            if diarize and not native_diarization:
                speaker_backend = ba.PyannoteBackend()
            # Always pair the ASR with the BA2 CHATUtterance BERT
            # segmenter — every transcribe path must produce one
            # utterance per sentence. ChatWhisper does segmentation
            # internally; for it we leave utseg_backend=None so the
            # recipe takes the fast path.
            utseg_backend: Any = None
            if not _engine_self_segments(engine):
                utseg_backend = _build_utseg(ba, lang_code, engine)
            pipeline = ba.recipes.transcribe(
                asr_backend=asr_backend,
                speaker_backend=speaker_backend,
                utseg_backend=utseg_backend,
                workers=opts.workers,
            )
            inputs, root = collect_media_inputs(folder, language=lang_code.alpha_3)
            for inp in inputs:
                ui.push(Task.from_input(inp))
            list(
                ui.run_pipeline(
                    pipeline,
                    inputs,
                    on_outcome=lambda outcome: write_outcome(
                        outcome,
                        root,
                        out,
                        output_suffix=".cha",
                        strip_word_timing=nowor,
                    ),
                )
            )

        raise typer.Exit(code=ui.exit_code)


def _build_asr(
    ba: Any,
    engine: AsrEngine,
    model: str | None,
    lang: LanguageCode,
    num_speakers: int = 2,
    device: str | None = None,
):
    """Construct the ASR backend for `engine`. Returns (backend, native_diarization).

    Every backend takes the resolved `LanguageCode` directly. Each
    one's constructor pulls whichever form (`alpha_2`, `alpha_3`, or
    `name`) its underlying SDK expects.
    """
    m = model or _DEFAULT_MODEL.get(engine)
    if engine is AsrEngine.rev:
        return ba.RevAI(language=lang, num_speakers=num_speakers), True
    if engine is AsrEngine.google:
        return ba.GoogleGenAIBackend(
            language=lang,
            model=m or "gemini-3.5-flash",
            num_speakers=num_speakers,
        ), True
    if engine is AsrEngine.whisper:
        return ba.WhisperBackend(model=m, language=lang, device=device), False
    if engine is AsrEngine.chatwhisper:
        return ba.ChatWhisperBackend(language=lang, device=device), False
    if engine is AsrEngine.openai:
        return ba.OpenAIWhisperBackend(model=m, language=lang, device=device), False
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
    if engine is AsrEngine.aliyun:
        # Aliyun NLS Cloud (BA2 AlibabaEngine). Language is pinned by the
        # configured AppKey, not the CLI arg — Aliyun NLS requires a
        # per-language project. Credentials in ~/.batchalign.ini
        # (`engine.aliyun.{ak_id,ak_secret,ak_appkey}`).
        return ba.AliyunAsrBackend(), False
    if engine is AsrEngine.malayalam:
        return ba.MalayalamWav2Vec2Backend(
            model=m or "gvs/wav2vec2-large-xlsr-malayalam",
            device=device,
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
    if lang.alpha_3 == "mal":
        return ba.MalayalamSaTBackend()
    key = _UTSEG_LANG_3.get(lang.alpha_3)
    if key is None:
        return None
    try:
        cantonese = engine is AsrEngine.funaudio
        return ba.CHATUtteranceBackend(lang=key, cantonese_inference=cantonese)
    except ValueError:
        return None
