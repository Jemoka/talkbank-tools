"""Shared runtime constants and helpers for the Python worker/runtime layer.

This module is consumed by the Python worker and pipeline code and avoids
heavy imports at module import time.  It provides:

- **Runtime release metadata**: ``VERSION_NUMBER``, ``RELEASE_DATE``,
  ``RELEASE_NOTES`` read from ``batchalign/version`` at import time. This data
  is user-visible runtime metadata, not the canonical package-version source.
- **Command/task mapping**: ``Cmd2Task``, ``KNOWN_ENGINE_KEYS``,
  ``COMMAND_BASE_MB``.
- **Free-threaded detection**: ``FREE_THREADED`` flag and ``is_free_threaded()``
  for selecting ``ThreadPoolExecutor`` vs ``ProcessPoolExecutor``.
- **Command classification**: ``PROCESS_COMMANDS``, ``GPU_HEAVY_COMMANDS``.
- **Memory tuning**: ``MAX_GPU_WORKERS``, ``MAX_PROCESS_WORKERS``,
  ``MAX_THREAD_WORKERS``, per-command memory budgets.
- **System RAM helpers**: ``system_ram_mb()``, ``available_memory_mb()``.

Constants marked "# from TOML" are loaded from ``runtime_constants.toml`` — the
single source of truth shared with the Rust server.
"""

from __future__ import annotations

import os
import platform
import sys
import tomllib
from pathlib import Path

# ---------------------------------------------------------------------------
# Load shared constants from TOML (single source of truth with Rust)
# ---------------------------------------------------------------------------

_TOML_PATH = Path(__file__).resolve().parent / "runtime_constants.toml"
with open(_TOML_PATH, "rb") as _tf:
    _TOML = tomllib.load(_tf)

# ---------------------------------------------------------------------------
# Runtime release metadata (read once from batchalign/version at import time)
# ---------------------------------------------------------------------------

_version_path = Path(__file__).parent / "version"
with open(_version_path, "r", encoding="utf-8") as _vf:
    VERSION_NUMBER, RELEASE_DATE, RELEASE_NOTES = [
        line.strip() for line in _vf.readlines()[:3]
    ]


# ---------------------------------------------------------------------------
# Command ↔ task mapping (from TOML)
# ---------------------------------------------------------------------------

Cmd2Task: dict[str, str] = dict(_TOML["cmd2task"])

# kwargs that are engine-override keys (passed via --engine-overrides),
# NOT per-file processing kwargs.
KNOWN_ENGINE_KEYS: frozenset[str] = frozenset(_TOML["known_engine_keys"]["keys"])

# ---------------------------------------------------------------------------
# Free-threaded Python detection
# ---------------------------------------------------------------------------


def is_free_threaded() -> bool:
    """Check if running on free-threaded Python (GIL disabled).

    Requires Python 3.13+ free-threading build with ``PYTHON_GIL=0`` or
    ``-Xgil=0``.  Returns False on regular CPython.
    """
    return hasattr(sys, "_is_gil_enabled") and not sys._is_gil_enabled()


FREE_THREADED: bool = is_free_threaded()
"""bool : ``True`` when running on free-threaded CPython with the GIL disabled.

Computed once at import time.  When ``True``, CPU-bound commands like
morphotag and utseg use ``ThreadPoolExecutor`` (shared models, ~1 GB) instead
of ``ProcessPoolExecutor`` (duplicated models, ~8 GB each).
"""

# ---------------------------------------------------------------------------
# Command classification (from TOML)
# ---------------------------------------------------------------------------

# CPU-bound commands benefit from ProcessPoolExecutor (true parallelism).
# On free-threaded Python, morphotag and utseg can use ThreadPoolExecutor
# with shared models — one copy of Stanza instead of one per process.
PROCESS_COMMANDS: frozenset[str] = frozenset(
    _TOML["process_commands"]["free_threaded"]
    if FREE_THREADED
    else _TOML["process_commands"]["gil"]
)

# GPU-bound commands where MPS/CUDA is the bottleneck, not CPU.
GPU_HEAVY_COMMANDS: frozenset[str] = frozenset(
    _TOML["gpu_heavy_commands"]["commands"]
)

# ---------------------------------------------------------------------------
# Memory tuning constants (from TOML)
# ---------------------------------------------------------------------------

MAX_GPU_WORKERS: int = _TOML["worker_caps"]["max_gpu_workers"]
MAX_PROCESS_WORKERS: int = _TOML["worker_caps"]["max_process_workers"]
MAX_THREAD_WORKERS: int = _TOML["worker_caps"]["max_thread_workers"]

# Per-command base memory (MB) for workers.
COMMAND_BASE_MB: dict[str, int] = dict(
    _TOML["command_base_mb"]["threaded"]
    if FREE_THREADED
    else _TOML["command_base_mb"]["process"]
)
DEFAULT_BASE_MB: int = _TOML["memory"]["default_base_mb"]
MB_PER_FILE_MB: int = _TOML["memory"]["mb_per_file_mb"]
LOADING_OVERHEAD: float = _TOML["memory"]["loading_overhead"]

# ---------------------------------------------------------------------------
# System RAM helpers
# ---------------------------------------------------------------------------


def system_ram_mb() -> int | None:
    """Read total physical system RAM in megabytes.

    Returns
    -------
    int or None
        Total RAM in MB, or ``None`` if the value cannot be determined
        (e.g. on platforms that lack ``os.sysconf``).
    """
    try:
        return os.sysconf("SC_PAGE_SIZE") * os.sysconf("SC_PHYS_PAGES") // (1024 * 1024)
    except (ValueError, OSError, AttributeError):
        return None


def available_memory_mb() -> int | None:
    """Read *available* system RAM in MB, or None if unavailable.

    - **Linux**: ``/proc/meminfo`` → ``MemAvailable`` (kernel-computed, accurate).
    - **macOS**: ``psutil.virtual_memory().available`` with a 30% haircut —
      macOS counts compressed pages as "available", over-reporting by ~30%.
    - **Fallback**: ``None`` — callers should use ``system_ram_mb()`` minus a
      static reserve.
    """
    system = platform.system()

    if system == "Linux":
        try:
            with open("/proc/meminfo", "r") as f:
                for line in f:
                    if line.startswith("MemAvailable:"):
                        # Value is in kB
                        return int(line.split()[1]) // 1024
        except (OSError, ValueError, IndexError):
            pass
        return None

    if system == "Darwin":
        try:
            import psutil
            avail = psutil.virtual_memory().available
            return int(avail * 0.70) // (1024 * 1024)
        except (ImportError, OSError, AttributeError):
            return None

    return None
