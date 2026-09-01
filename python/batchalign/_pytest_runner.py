"""Bazel-side pytest launcher.

`py_test` requires a single Python entry point; pytest itself is the
collector. Bazel starts tests from the source workspace, so an unresolved
``python/batchalign/tests`` argument makes pytest import the checkout rather
than the runfiles package containing the compiled ``_core`` extension. This
launcher anchors relative arguments and the working directory to the runfiles
workspace before collection.
"""

from __future__ import annotations

import os
import sys
from pathlib import Path

import pytest


if __name__ == "__main__":
    runfiles_workspace = (
        Path(os.environ["TEST_SRCDIR"]) / os.environ["TEST_WORKSPACE"]
    )
    os.chdir(runfiles_workspace)
    pytest_args = [
        str(runfiles_workspace / arg) if not arg.startswith("-") else arg
        for arg in sys.argv[1:]
    ]
    sys.exit(pytest.main(pytest_args))
