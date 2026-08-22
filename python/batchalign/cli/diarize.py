"""Standalone speaker diarization to deterministic turns JSON."""

from __future__ import annotations

import base64
import json
from pathlib import Path
from typing import Any

import typer

from ._common import MEDIA_EXTENSIONS, _root_for, _walk, safe_resolve


def _protocol_audio(audio: Any) -> Any:
    """Bridge native ``prepare_audio`` output to the Python wire model."""
    from batchalign._core.proto import PreparedAudio

    return PreparedAudio(
        pcm_f32le=base64.b64encode(bytes(audio.pcm_f32le)),
        sample_rate=int(audio.sample_rate),
        channels=int(audio.channels),
        frame_count=int(audio.frame_count),
    )


def _turns_document(output: Any) -> dict[str, Any]:
    """Project backend labels to stable anonymous CHAT-style track names."""
    segments = list(output.diarization.segments)
    labels = sorted({str(segment.speaker) for segment in segments})
    tracks = {label: f"PAR{index}" for index, label in enumerate(labels)}
    return {
        "source": "batchalign3:pyannote-ai",
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
        num_speakers: int = typer.Option(
            0,
            "--num-speakers",
            "-n",
            min=0,
            help="Expected speaker count; zero auto-detects.",
        ),
    ) -> None:
        """Detect anonymous speaker turns with pyannoteAI cloud."""
        import batchalign as ba
        from batchalign._core import prepare_audio
        from batchalign._core.proto import SpeakerInput
        from batchalign.inputs import media_from_path

        paths = _walk(folder, MEDIA_EXTENSIONS)
        root = _root_for(folder)
        backend = ba.PyannoteAIBackend(num_speakers=num_speakers)
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
                json.dumps(_turns_document(response), indent=2) + "\n",
                encoding="utf-8",
            )


__all__ = ["_turns_document", "register"]
