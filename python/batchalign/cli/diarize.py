"""Standalone speaker diarization to deterministic turns JSON."""

from __future__ import annotations

import base64
import json
from enum import Enum
from pathlib import Path
from typing import Any

import typer

from ._common import MEDIA_EXTENSIONS, _root_for, _walk, safe_resolve


class DiarizeEngine(str, Enum):
    """Speaker diarization backend exposed by the CLIs."""

    pyannote_ai = "pyannote-ai"
    pyannote = "pyannote"


def _build_backend(ba: Any, engine: DiarizeEngine, num_speakers: int) -> Any:
    if engine is DiarizeEngine.pyannote_ai:
        return ba.PyannoteAIBackend(num_speakers=num_speakers)
    return ba.PyannoteBackend(num_speakers=num_speakers)


def _protocol_audio(audio: Any) -> Any:
    """Bridge native ``prepare_audio`` output to the Python wire model."""
    from batchalign._core.proto import PreparedAudio

    return PreparedAudio(
        pcm_f32le=base64.b64encode(bytes(audio.pcm_f32le)),
        sample_rate=int(audio.sample_rate),
        channels=int(audio.channels),
        frame_count=int(audio.frame_count),
    )


def _turns_document(
    output: Any,
    engine: DiarizeEngine = DiarizeEngine.pyannote_ai,
) -> dict[str, Any]:
    """Project backend labels to stable anonymous CHAT-style track names."""
    segments = list(output.diarization.segments)
    labels = sorted({str(segment.speaker) for segment in segments})
    tracks = {label: f"PAR{index}" for index, label in enumerate(labels)}
    return {
        "source": f"batchalign3:{engine.value}",
        "turns": [
            {
                "start_ms": int(segment.start_ms),
                "end_ms": int(segment.end_ms),
                "track": tracks[str(segment.speaker)],
            }
            for segment in segments
        ],
    }


def _target(path: Path, root: Path, out: Path | None) -> Path:
    if out is None:
        return path.with_suffix(".turns.json")
    target = out / path.relative_to(root)
    target = target.with_suffix(".turns.json")
    target.parent.mkdir(parents=True, exist_ok=True)
    safe_resolve(target.parent, out.resolve())
    return target


def register(app: typer.Typer) -> None:
    @app.command()
    def diarize(
        folder: Path = typer.Argument(
            ...,
            exists=True,
            help="Media file or folder to scan recursively.",
        ),
        out: Path | None = typer.Option(
            None,
            "--out",
            "-o",
            help="Output folder; default writes .turns.json beside each input.",
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
        """Detect speaker turns with pyannoteAI cloud or local Pyannote."""
        import batchalign as ba
        from batchalign._core import prepare_audio
        from batchalign._core.proto import SpeakerInput
        from batchalign.inputs import media_from_path

        paths = _walk(folder, MEDIA_EXTENSIONS)
        root = _root_for(folder)
        backend = _build_backend(ba, engine, num_speakers)
        for path in paths:
            media = media_from_path(path, source_id=str(path))
            request = SpeakerInput(
                source_id=str(path),
                audio=_protocol_audio(prepare_audio(media)),
                num_speakers=num_speakers,
            )
            response = backend.call([request])[0]
            target = _target(path, root, out)
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(
                json.dumps(_turns_document(response, engine), indent=2) + "\n",
                encoding="utf-8",
            )


__all__ = ["DiarizeEngine", "_build_backend", "_turns_document", "register"]
