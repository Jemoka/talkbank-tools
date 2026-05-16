"""Bazel-side pytest launcher.

`py_test` requires a single Python entry point; pytest itself is the
collector. This shim hands `sys.argv` (set by the rule's `args`) to
`pytest.main()` so `bazel test //python/batchalign:tests` runs the same
collection as a manual `pytest python/batchalign/tests` invocation.
"""

from __future__ import annotations

import sys

import pytest


if __name__ == "__main__":
    sys.exit(pytest.main(sys.argv[1:]))
