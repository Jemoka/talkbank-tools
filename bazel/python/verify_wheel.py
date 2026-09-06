"""Reject Batchalign wheels containing development-only source-tree files."""

from __future__ import annotations

import sys
from pathlib import Path
from zipfile import ZipFile


wheel_paths = [Path(argument) for argument in sys.argv[1:]]
if not wheel_paths:
    raise SystemExit("usage: verify_wheel.py WHEEL [WHEEL ...]")

for wheel_path in wheel_paths:
    with ZipFile(wheel_path) as wheel:
        members = wheel.namelist()

    forbidden = [
        member
        for member in members
        if "__pycache__" in member
        or member.endswith(".pyc")
        or member.endswith("/BUILD.bazel")
        or member.startswith("batchalign/tests/")
    ]
    if forbidden:
        rendered = "\n".join(f"  {member}" for member in forbidden)
        raise SystemExit(f"{wheel_path} contains development-only files:\n{rendered}")

    print(f"verified clean wheel: {wheel_path}")
