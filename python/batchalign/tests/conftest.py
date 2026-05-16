"""Shared pytest fixtures for batchalign Python tests.

Most tests should run without the compiled `_core` extension. Where a
test depends on the .so, it should `pytest.importorskip("batchalign._core")`.
"""

from __future__ import annotations

import sys
from pathlib import Path

import pytest


# Make `python/` importable when pytest is invoked from a different cwd
# (e.g. from the repo root via `pytest python/batchalign/tests`).
_PYTHON_ROOT = Path(__file__).resolve().parents[2]
if str(_PYTHON_ROOT) not in sys.path:
    sys.path.insert(0, str(_PYTHON_ROOT))


@pytest.fixture
def core_available() -> bool:
    """Whether the compiled `batchalign._core` extension is importable.

    Tests that require it should `pytest.skip()` when this is False.
    """
    try:
        import batchalign._core  # noqa: F401
        return True
    except ImportError:
        return False


@pytest.fixture
def tmp_ini(tmp_path: Path) -> Path:
    """Yield a temporary `.batchalign.ini` path; tests write into it."""
    return tmp_path / ".batchalign.ini"
