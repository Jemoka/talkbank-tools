"""Python-side helpers around `MediaInput`, `ChatInput`, `AiChatInput`, `PairedInput`.

The authoritative types live in the compiled `batchalign._core`
extension — they're PyO3 classes constructed from Rust. Helpers here
add idiomatic Python conveniences (`from_path`, directory iteration)
without re-implementing semantics.

When `_core` is not yet built, the helpers fall back to lightweight
dataclasses so import-time tests can still collect.
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, Iterator

try:
    from batchalign._core import (  # type: ignore[attr-defined]
        MediaInput as _CoreMediaInput,
        ChatInput as _CoreChatInput,
        AiChatInput as _CoreAiChatInput,
        PairedInput as _CorePairedInput,
    )
    _CORE_OK = True
except ImportError:
    _CORE_OK = False

    @dataclass
    class _CoreMediaInput:  # type: ignore[no-redef]
        path: str
        source_id: str = ""
        language: str | None = None

    @dataclass
    class _CoreChatInput:  # type: ignore[no-redef]
        path: str
        source_id: str = ""

    @dataclass
    class _CoreAiChatInput:  # type: ignore[no-redef]
        path: str
        source_id: str = ""
        instruction: str = ""

    @dataclass
    class _CorePairedInput:  # type: ignore[no-redef]
        main: str
        gold: str
        source_id: str = ""


def media_from_path(
    path: str | Path,
    source_id: str | None = None,
    language: str | None = None,
) -> _CoreMediaInput:
    """Construct a `MediaInput` from a filesystem path.

    `source_id` defaults to the file stem (matches the spec §16.4 CLI default).
    """
    p = Path(path).absolute()
    sid = source_id if source_id is not None else p.stem
    return _CoreMediaInput(path=str(p), source_id=sid, language=language)


def chat_from_path(path: str | Path, source_id: str | None = None) -> _CoreChatInput:
    """Construct a `ChatInput` from a filesystem path."""
    p = Path(path).absolute()
    sid = source_id if source_id is not None else p.stem
    return _CoreChatInput(path=str(p), source_id=sid)


def ai_from_path(
    path: str | Path,
    *,
    instruction: str,
    source_id: str | None = None,
) -> _CoreAiChatInput:
    """Construct an AI transcript input from a CHAT path plus instruction."""
    p = Path(path).absolute()
    sid = source_id if source_id is not None else p.stem
    return _CoreAiChatInput(path=str(p), source_id=sid, instruction=instruction)


def paired_from_paths(
    main: str | Path,
    gold: str | Path,
    source_id: str | None = None,
) -> _CorePairedInput:
    """Construct a `PairedInput` (main + gold transcript) for the Compare task."""
    main_p = Path(main).absolute()
    gold_p = Path(gold).absolute()
    sid = source_id if source_id is not None else main_p.stem
    return _CorePairedInput(main=str(main_p), gold=str(gold_p), source_id=sid)


def sibling_media_for_chat(
    chat_path: str | Path,
    *,
    extensions: Iterable[str] = (".wav", ".mp3", ".m4a", ".flac", ".ogg", ".mp4"),
) -> Path | None:
    """Locate the sibling media file for a CHAT transcript.

    Strategy:
      1. Read the `@Media:` header from the CHAT file (BA2 convention:
         filename without extension); probe each `extensions` suffix
         alongside the CHAT file.
      2. Fall back to a stem-match against the CHAT file's own stem
         (`/path/to/foo.cha` → `/path/to/foo.wav`).

    Returns the resolved Path on success, `None` when no candidate
    exists. Used by the align CLI when the user passes a directory or a
    single CHAT file and wants the engine to auto-find the audio.

    Landing 3 #15 from the BA3 cutover plan.
    """
    p = Path(chat_path)
    if not p.is_file():
        return None
    media_stem: str | None = None
    try:
        with p.open("r", encoding="utf-8") as f:
            for line in f:
                if line.startswith("@Media:"):
                    body = line.split(":", 1)[1].strip()
                    media_stem = body.split(",", 1)[0].strip()
                    break
    except OSError:
        media_stem = None
    candidates: list[Path] = []
    if media_stem:
        for ext in extensions:
            candidates.append(p.parent / f"{media_stem}{ext}")
            candidates.append(p.parent / f"{media_stem}{ext.upper()}")
    for ext in extensions:
        candidates.append(p.with_suffix(ext))
        candidates.append(p.with_suffix(ext.upper()))
    for cand in candidates:
        if cand.is_file():
            return cand
    return None


def iter_media(
    root: str | Path,
    *,
    extensions: Iterable[str] = (".wav", ".mp3", ".m4a", ".flac", ".ogg", ".mp4"),
) -> Iterator[_CoreMediaInput]:
    """Walk `root` for media files and yield `MediaInput` instances.

    `extensions` is case-insensitive. Filenames whose stem starts with `.`
    are skipped (hidden/macOS dotfiles).
    """
    root_p = Path(root)
    if root_p.is_file():
        yield media_from_path(root_p)
        return
    suffixes = {ext.lower() for ext in extensions}
    for path in sorted(root_p.rglob("*")):
        if not path.is_file():
            continue
        if path.name.startswith("."):
            continue
        if path.suffix.lower() in suffixes:
            yield media_from_path(path)


def iter_chat(
    root: str | Path,
    *,
    extensions: Iterable[str] = (".cha", ".chat"),
) -> Iterator[_CoreChatInput]:
    """Walk `root` for CHAT transcript files and yield `ChatInput` instances."""
    root_p = Path(root)
    if root_p.is_file():
        yield chat_from_path(root_p)
        return
    suffixes = {ext.lower() for ext in extensions}
    for path in sorted(root_p.rglob("*")):
        if not path.is_file():
            continue
        if path.name.startswith("."):
            continue
        if path.suffix.lower() in suffixes:
            yield chat_from_path(path)


# Re-export the type names so callers can do `from batchalign.inputs import MediaInput`.
MediaInput = _CoreMediaInput
ChatInput = _CoreChatInput
PairedInput = _CorePairedInput
AiChatInput = _CoreAiChatInput

__all__ = [
    "MediaInput",
    "ChatInput",
    "PairedInput",
    "AiChatInput",
    "media_from_path",
    "chat_from_path",
    "ai_from_path",
    "paired_from_paths",
    "sibling_media_for_chat",
    "iter_media",
    "iter_chat",
]
