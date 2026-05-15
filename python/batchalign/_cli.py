"""Python-owned console entry point for ``batchalign3``.

Locates and execs the standalone Rust CLI binary. The binary is either:
1. Packaged inside the wheel at ``python/batchalign/_bin/batchalign3`` (PyPI install)
2. Pre-built at ``target/{debug,release}/batchalign3`` (source checkout)
3. Compiled on the fly via ``cargo run`` (source checkout, not yet built)
"""

from __future__ import annotations

import os
import shutil
import sys
from pathlib import Path

_BIN_NAME = "batchalign3.exe" if sys.platform == "win32" else "batchalign3"


def _prime_runtime_env(binary: Path | None = None) -> None:
    """Propagate runtime discovery hints into the Rust CLI process."""
    if "BATCHALIGN_PYTHON" not in os.environ:
        os.environ["BATCHALIGN_PYTHON"] = sys.executable
    if binary is not None and "BATCHALIGN_SELF_EXE" not in os.environ:
        os.environ["BATCHALIGN_SELF_EXE"] = str(binary)


def _exec_binary(binary: Path) -> None:
    """Replace this process with the Rust binary.

    Propagates the current Python interpreter via BATCHALIGN_PYTHON so the
    Rust binary can spawn workers using the same venv that has batchalign
    installed. Without this, the binary would fall back to system Python
    which doesn't have the batchalign package.
    """
    _prime_runtime_env(binary)

    if sys.platform == "win32":
        import subprocess

        raise SystemExit(subprocess.call([str(binary), *sys.argv[1:]]))
    os.execv(str(binary), [str(binary), *sys.argv[1:]])


def _repo_root() -> Path | None:
    """Return the repo root when running from a checkout, else ``None``."""
    root = Path(__file__).resolve().parent.parent
    if (root / "Cargo.toml").exists() and (root / "crates" / "batchalign").exists():
        return root
    return None


def main() -> None:
    """Run the installed ``batchalign3`` command."""

    # 1. Packaged binary (PyPI / wheel install)
    packaged = Path(__file__).resolve().parent / "_bin" / _BIN_NAME
    if packaged.is_file():
        _exec_binary(packaged)

    # 2. Dev checkout: pre-built binary
    root = _repo_root()
    if root is not None:
        for profile in ("debug", "release"):
            candidate = root / "target" / profile / _BIN_NAME
            if candidate.is_file():
                _exec_binary(candidate)

        # 3. Dev checkout: compile on the fly
        cargo = shutil.which("cargo")
        if cargo:
            _prime_runtime_env()
            os.execvp(
                cargo,
                [
                    cargo,
                    "run",
                    "-q",
                    "-p",
                    "batchalign",
                    "--bin",
                    "batchalign3",
                    "--",
                    *sys.argv[1:],
                ],
            )

    raise SystemExit(
        "batchalign3 CLI binary not found. Reinstall batchalign3 or, "
        "in a source checkout, run `cargo build -p batchalign`."
    )
