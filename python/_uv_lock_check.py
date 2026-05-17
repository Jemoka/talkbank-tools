"""`uv lock --check` as a Bazel py_test.

Fails CI when `python/pyproject.toml` has drifted from `python/uv.lock`.
Exists because Bazel reads `uv.lock` at module-resolution time and
cannot itself regenerate the lockfile during a build — the regen is
a `uv lock` invocation. This test gates drift so PRs that touch
pyproject.toml without re-locking are caught immediately rather than
producing silently-stale Bazel deps.

When this test fails the fix is one command:

    cd python && uv lock

then commit `python/uv.lock`. No other action needed.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys


def _resolve_runfile(path: str) -> str:
    # The runfiles tree mirrors the workspace; resolve relative to the
    # py_test binary's runfiles root.
    runfiles = os.environ.get("RUNFILES_DIR") or os.environ.get("TEST_SRCDIR")
    if runfiles:
        candidate = os.path.join(runfiles, "_main", "python", path)
        if os.path.exists(candidate):
            return candidate
    # Fall back to repo-relative (interactive use).
    if os.path.exists(os.path.join("python", path)):
        return os.path.join("python", path)
    raise FileNotFoundError(f"could not locate python/{path} in runfiles or workspace")


def main() -> int:
    pyproject = _resolve_runfile("pyproject.toml")
    lock = _resolve_runfile("uv.lock")
    workdir = os.path.dirname(pyproject)

    uv = shutil.which("uv")
    if uv is None:
        print(
            "uv_lock_check: `uv` binary not found on PATH; skipping. "
            "Install uv (https://docs.astral.sh/uv/) so this test can run.",
            file=sys.stderr,
        )
        return 0

    res = subprocess.run([uv, "lock", "--check"], cwd=workdir, capture_output=True, text=True)
    if res.returncode != 0:
        print("uv_lock_check: uv.lock is stale relative to pyproject.toml.", file=sys.stderr)
        print(res.stderr, file=sys.stderr)
        print("fix: `cd python && uv lock`, then commit python/uv.lock", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
