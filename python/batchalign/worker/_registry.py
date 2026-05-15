"""Worker registry file I/O for persistent TCP workers.

The registry file (``workers.json``) is the discovery mechanism between
independently started worker daemons and the Rust server. Each worker writes
its own entry on startup and removes it on shutdown. The server reads the
registry to discover pre-started workers, health-checks each one, and removes
stale entries (workers that crashed without cleanup).

File locking uses ``fcntl.flock`` on Unix and ``msvcrt.locking`` on Windows
to prevent concurrent writers from corrupting the JSON array.

Registry path: ``~/.batchalign3/workers.json`` (configurable via
``BATCHALIGN_STATE_DIR``).
"""

from __future__ import annotations

import json
import logging
import os
import sys
from dataclasses import asdict, dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import IO, TYPE_CHECKING

if TYPE_CHECKING:
    from batchalign.inference._domain_types import LanguageCode, TcpPort

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Registry entry
# ---------------------------------------------------------------------------


@dataclass(frozen=True, slots=True)
class WorkerRegistryEntry:
    """One worker's entry in the registry file."""

    pid: int
    host: str
    port: TcpPort
    profile: str
    lang: LanguageCode
    engine_overrides: str = ""
    ownership: str = "external"
    owner_server_instance_id: str = ""
    owner_server_pid: int | None = None
    started_at: str = field(default_factory=lambda: datetime.now(timezone.utc).isoformat())


# ---------------------------------------------------------------------------
# Registry path resolution
# ---------------------------------------------------------------------------


def _default_registry_path() -> Path:
    """Resolve the default registry file path from environment."""
    state_dir = os.environ.get("BATCHALIGN_STATE_DIR", "")
    if state_dir.strip():
        return Path(state_dir) / "workers.json"
    home = Path.home()
    return home / ".batchalign3" / "workers.json"


# ---------------------------------------------------------------------------
# File-locked read/write helpers
# ---------------------------------------------------------------------------


_IS_WINDOWS = sys.platform == "win32"


def _lock_file(f: IO[str]) -> None:
    """Acquire an exclusive lock on the file descriptor."""
    fd = f.fileno()
    if _IS_WINDOWS:
        import msvcrt

        msvcrt.locking(fd, msvcrt.LK_LOCK, 1)  # type: ignore[attr-defined]
    else:
        import fcntl

        fcntl.flock(fd, fcntl.LOCK_EX)


def _unlock_file(f: IO[str]) -> None:
    """Release the lock on the file descriptor."""
    fd = f.fileno()
    if _IS_WINDOWS:
        import msvcrt

        msvcrt.locking(fd, msvcrt.LK_NBLCK, 1)  # type: ignore[attr-defined]
    else:
        import fcntl

        fcntl.flock(fd, fcntl.LOCK_UN)


def _read_entries(registry_path: Path) -> list[WorkerRegistryEntry]:
    """Read all entries from the registry file (no locking)."""
    if not registry_path.exists():
        return []
    try:
        raw = json.loads(registry_path.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError) as exc:
        logger.warning("Failed to read worker registry %s: %s", registry_path, exc)
        return []
    if not isinstance(raw, list):
        return []
    entries: list[WorkerRegistryEntry] = []
    for item in raw:
        if isinstance(item, dict):
            try:
                entries.append(WorkerRegistryEntry(**item))
            except TypeError:
                continue
    return entries


def _write_entries(registry_path: Path, entries: list[WorkerRegistryEntry]) -> None:
    """Write all entries to the registry file atomically (caller holds lock)."""
    registry_path.parent.mkdir(parents=True, exist_ok=True)
    data = json.dumps([asdict(e) for e in entries], indent=2) + "\n"
    # Write to temp file then rename for atomicity.
    tmp_path = registry_path.with_suffix(".tmp")
    tmp_path.write_text(data, encoding="utf-8")
    tmp_path.replace(registry_path)


# ---------------------------------------------------------------------------
# Public API
# ---------------------------------------------------------------------------


def register_worker(
    entry: WorkerRegistryEntry,
    *,
    registry_path: Path | None = None,
) -> None:
    """Add a worker entry to the registry file.

    If an entry with the same ``(host, port)`` already exists, it is replaced.
    """
    path = registry_path or _default_registry_path()
    path.parent.mkdir(parents=True, exist_ok=True)

    # Open for read+write, creating if needed.
    with open(path, "a+", encoding="utf-8") as f:
        _lock_file(f)
        try:
            f.seek(0)
            content = f.read()
            if content.strip():
                try:
                    raw = json.loads(content)
                    entries = [
                        WorkerRegistryEntry(**item)
                        for item in raw
                        if isinstance(item, dict)
                    ]
                except (json.JSONDecodeError, TypeError):
                    entries = []
            else:
                entries = []

            # Replace existing entry for same (host, port).
            entries = [
                e for e in entries if not (e.host == entry.host and e.port == entry.port)
            ]
            entries.append(entry)
            _write_entries(path, entries)
        finally:
            _unlock_file(f)

    logger.info(
        "Registered worker pid=%d at %s:%d in %s",
        entry.pid,
        entry.host,
        entry.port,
        path,
    )


def unregister_worker(
    *,
    host: str,
    port: TcpPort,
    registry_path: Path | None = None,
) -> bool:
    """Remove a worker entry from the registry file.

    Returns ``True`` if an entry was removed, ``False`` if not found.
    """
    path = registry_path or _default_registry_path()
    if not path.exists():
        return False

    with open(path, "a+", encoding="utf-8") as f:
        _lock_file(f)
        try:
            f.seek(0)
            content = f.read()
            if not content.strip():
                return False
            try:
                raw = json.loads(content)
                entries = [
                    WorkerRegistryEntry(**item)
                    for item in raw
                    if isinstance(item, dict)
                ]
            except (json.JSONDecodeError, TypeError):
                return False

            before = len(entries)
            entries = [
                e for e in entries if not (e.host == host and e.port == port)
            ]
            if len(entries) == before:
                return False
            _write_entries(path, entries)
            return True
        finally:
            _unlock_file(f)


def list_workers(
    *,
    registry_path: Path | None = None,
) -> list[WorkerRegistryEntry]:
    """Read all worker entries from the registry file."""
    path = registry_path or _default_registry_path()
    return _read_entries(path)


def remove_stale_entry(
    *,
    pid: int,
    registry_path: Path | None = None,
) -> bool:
    """Remove a worker entry by PID (for crash cleanup).

    Returns ``True`` if an entry was removed.
    """
    path = registry_path or _default_registry_path()
    if not path.exists():
        return False

    with open(path, "a+", encoding="utf-8") as f:
        _lock_file(f)
        try:
            f.seek(0)
            content = f.read()
            if not content.strip():
                return False
            try:
                raw = json.loads(content)
                entries = [
                    WorkerRegistryEntry(**item)
                    for item in raw
                    if isinstance(item, dict)
                ]
            except (json.JSONDecodeError, TypeError):
                return False

            before = len(entries)
            entries = [e for e in entries if e.pid != pid]
            if len(entries) == before:
                return False
            _write_entries(path, entries)
            return True
        finally:
            _unlock_file(f)
