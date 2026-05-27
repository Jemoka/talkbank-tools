#!/usr/bin/env python3
"""Generate `_proto_generated.py` from a JSON Schema.

Wraps `datamodel-code-generator` and applies two small post-processing fixes:

1. **`Base64Str` → `Base64Bytes`.** datamodel-codegen maps JSON Schema
   `format: byte` to `pydantic.Base64Str`, which keeps the field as a string.
   Every batchalign backend calls `np.frombuffer(audio.pcm_f32le, …)`, which
   needs real bytes — so we swap to `pydantic.Base64Bytes` (auto-decodes the
   base64 payload at validation time).

2. **Drop the `Model(RootModel[Any])` placeholder.** datamodel-codegen always
   emits a root model for the top-level schema. Our schema has only `$defs`
   (no root content), so the placeholder is dead weight and re-exports `Any`
   under a misleading name. We strip it.

This script is invoked by the Bazel `proto_py_generated` genrule. Do not call
it from `just` recipes — the user explicitly does not want a manual gen step.

Usage:
    codegen_proto.py <input.schema.json> <output.py>
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path


def main() -> int:
    if len(sys.argv) != 3:
        print(f"usage: {sys.argv[0]} <input.schema.json> <output.py>", file=sys.stderr)
        return 2

    schema_path = Path(sys.argv[1]).resolve()
    output_path = Path(sys.argv[2]).resolve()

    cmd = [
        sys.executable,
        "-m",
        "datamodel_code_generator",
        "--input",
        str(schema_path),
        "--input-file-type",
        "jsonschema",
        "--output",
        str(output_path),
        "--output-model-type",
        "pydantic_v2.BaseModel",
        "--target-python-version",
        "3.12",
        "--use-standard-collections",
        "--use-union-operator",
        # `--use-annotated` emits `Annotated[int, Field(ge=0, le=65535)]` instead of
        # `conint(ge=0, le=65535)`. The latter is a function call that pydantic
        # can't resolve when type hints are strings (which they are under
        # `from __future__ import annotations`, the default in codegen output),
        # producing `class-not-fully-defined` errors at first model construction.
        "--use-annotated",
        "--use-double-quotes",
        "--use-schema-description",
        "--use-field-description",
        "--use-title-as-name",
        "--collapse-root-models",
        "--disable-timestamp",
        "--custom-file-header",
        (
            "# AUTO-GENERATED — DO NOT EDIT.\n"
            "#\n"
            "# Regenerated on every Bazel build from\n"
            "# crates/batchalign/batchalign-core/src/proto/*.rs via\n"
            "# bazel/python/codegen_proto.py. Edit the Rust source instead;\n"
            "# the next `bazel build` reflects your change here.\n"
            "#\n"
            "# Sibling helpers (rebuild_tagged_inputs / serialize_tagged_outputs)\n"
            "# live next to this file in proto.py."
        ),
    ]

    result = subprocess.run(cmd, check=False)
    if result.returncode != 0:
        return result.returncode

    text = output_path.read_text()

    # Swap Base64Str → Base64Bytes (import + every use).
    text = text.replace("Base64Str", "Base64Bytes")

    # Drop the dead `Model(RootModel[Any])` block. datamodel-codegen always
    # emits this for the document root; our schema has no root content, so
    # the class re-exports `Any` under a misleading name. Match the
    # `class Model(RootModel[Any]):` block including its `root: Any` body and
    # the two trailing blank lines.
    text = re.sub(
        r"(?m)^class Model\(RootModel\[Any\]\):\n(?:.+\n)+?\n\n",
        "",
        text,
        count=1,
    )

    # If RootModel is no longer used after we drop Model, prune the import
    # too — datamodel-codegen still imports it for the TaskInput/TaskOutput
    # union types, so leave it alone if those references survive.
    if "RootModel[" not in text:
        text = re.sub(r"(\bRootModel\b,?\s*)", "", text, count=1)

    # If `Any` is no longer referenced, prune that import too.
    if not re.search(r"\bAny\b", text.split("from typing import")[-1].split("\n", 1)[0]) \
            and "Any" not in text.replace("from typing import Any", "", 1):
        text = re.sub(r"from typing import Any\n", "", text, count=1)
        text = re.sub(r"(\bAny\b,\s*)", "", text, count=1)

    output_path.write_text(text)
    return 0


if __name__ == "__main__":
    sys.exit(main())
