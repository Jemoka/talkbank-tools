"""Path-escape guard tests for `cli._common.safe_resolve`.

Verifies that:
- a path under the root resolves cleanly,
- a symlinked path that points outside the root is rejected,
- a `..` traversal is rejected,
- an absolute path outside the root is rejected.
"""

from __future__ import annotations

import os
import tempfile
from pathlib import Path

import pytest
import typer

from batchalign.cli._common import safe_resolve


def test_under_root_resolves() -> None:
    with tempfile.TemporaryDirectory() as td:
        root = Path(td)
        target = root / "sub" / "file.cha"
        target.parent.mkdir()
        target.touch()
        assert safe_resolve(target, root) == target.resolve()


def test_dotdot_traversal_rejected() -> None:
    with tempfile.TemporaryDirectory() as td:
        root = Path(td) / "inside"
        root.mkdir()
        bad = root / ".." / "outside.cha"
        with pytest.raises(typer.BadParameter):
            safe_resolve(bad, root)


def test_absolute_outside_rejected() -> None:
    with tempfile.TemporaryDirectory() as td:
        root = Path(td) / "inside"
        root.mkdir()
        bad = Path("/etc/passwd")
        with pytest.raises(typer.BadParameter):
            safe_resolve(bad, root)


def test_symlink_outside_rejected() -> None:
    with tempfile.TemporaryDirectory() as td:
        outside = Path(td) / "outside"
        outside.mkdir()
        target = outside / "secret.cha"
        target.write_text("data")
        root = Path(td) / "inside"
        root.mkdir()
        link = root / "link.cha"
        os.symlink(target, link)
        with pytest.raises(typer.BadParameter):
            safe_resolve(link, root)
