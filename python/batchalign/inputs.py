"""Python-side helpers around `MediaInput`, `ChatInput`, `PairedInput`.

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
        PairedInput as _CorePairedInput,
    )
    _CORE_OK = True
except ImportError:
    _CORE_OK = False

    @dataclass
    class _CoreMediaInput:  # type: ignore[no-redef]
        path: str
        source_id: str = ""

    @dataclass
    class _CoreChatInput:  # type: ignore[no-redef]
        path: str
        source_id: str = ""

    @dataclass
    class _CorePairedInput:  # type: ignore[no-redef]
        main: str
        gold: str
        source_id: str = ""


def media_from_path(path: str | Path, source_id: str | None = None) -> _CoreMediaInput:
    """Construct a `MediaInput` from a filesystem path.

    `source_id` defaults to the file stem (matches the spec §16.4 CLI default).
    """
    p = Path(path).absolute()
    sid = source_id if source_id is not None else p.stem
    return _CoreMediaInput(path=str(p), source_id=sid)


def chat_from_path(path: str | Path, source_id: str | None = None) -> _CoreChatInput:
    """Construct a `ChatInput` from a filesystem path."""
    p = Path(path).absolute()
    sid = source_id if source_id is not None else p.stem
    return _CoreChatInput(path=str(p), source_id=sid)


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

__all__ = [
    "MediaInput",
    "ChatInput",
    "PairedInput",
    "media_from_path",
    "chat_from_path",
    "paired_from_paths",
    "iter_media",
    "iter_chat",
]
