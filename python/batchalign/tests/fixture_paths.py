"""Runfiles-aware paths for tracked Batchalign test fixtures."""

from pathlib import Path


def fixture_root(category: str) -> Path:
    relative = Path("resources/test_fixtures") / category
    for ancestor in Path(__file__).resolve().parents:
        candidate = ancestor / relative
        if candidate.is_dir():
            return candidate
    return Path.cwd() / relative
