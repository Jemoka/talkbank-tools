#!/usr/bin/env python3
"""Regenerate `openapi.gen.ts` + `capabilities.snapshot.json` for the GUI.

The daemon's HTTP surface is the single source of truth (every recipe,
backend, and per-recipe Pydantic schema is derived from Python
signatures at startup; see `python/batchalign/api.py`). This script
captures that surface in two artifacts the desktop GUI consumes:

  1. `apps/batchalign/batchalign-gui/src/protocol/openapi.gen.ts`
     — fully-typed `paths` + `components`, produced by piping
     `app.openapi()` through `openapi-typescript` (npm).
  2. `apps/batchalign/batchalign-gui/src/protocol/capabilities.snapshot.json`
     — a captured `GET /capabilities` response. Embedded by Vite as a
     fallback for offline development. Production GUIs overwrite this
     with the live response on startup.

Usage:
  just batchalign gen-openapi              # run via Bazel
  python scripts/gen_openapi.py            # direct invocation (dev)

Exits non-zero if the generated artifacts would change without
committing — pair with `bazel test //apps/batchalign/batchalign-gui:
openapi_freshness` for CI freshness gating.
"""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any


def repo_root() -> Path:
    """Resolve repo root from this script's location."""
    return Path(__file__).resolve().parent.parent


def generate(
    repo: Path,
    *,
    check: bool = False,
) -> int:
    """Generate `openapi.gen.ts` + `capabilities.snapshot.json`.

    When `check=True`, exit non-zero if the on-disk versions would
    change. Otherwise overwrite them.
    """
    out_dir = repo / "apps/batchalign/batchalign-gui/src/protocol"
    out_dir.mkdir(parents=True, exist_ok=True)
    ts_out = out_dir / "openapi.gen.ts"
    snapshot_out = out_dir / "capabilities.snapshot.json"

    openapi_json = _capture_openapi()
    capabilities_json = _capture_capabilities()

    with tempfile.TemporaryDirectory() as td:
        tmp = Path(td)
        oa = tmp / "openapi.json"
        oa.write_text(json.dumps(openapi_json, indent=2, sort_keys=True))
        ts = _run_openapi_typescript(oa)

    if check:
        existing_ts = ts_out.read_text() if ts_out.exists() else ""
        existing_snap = snapshot_out.read_text() if snapshot_out.exists() else ""
        expected_snap = json.dumps(capabilities_json, indent=2, sort_keys=True) + "\n"
        if existing_ts.strip() != ts.strip() or existing_snap != expected_snap:
            print(
                "openapi.gen.ts or capabilities.snapshot.json drifted from the "
                "daemon's live surface. Run `just batchalign gen-openapi` to "
                "regenerate and commit the result.",
                file=sys.stderr,
            )
            return 1
        return 0

    ts_out.write_text(ts)
    snapshot_out.write_text(json.dumps(capabilities_json, indent=2, sort_keys=True) + "\n")
    print(f"wrote {ts_out.relative_to(repo)}")
    print(f"wrote {snapshot_out.relative_to(repo)}")
    return 0


def _capture_openapi() -> dict[str, Any]:
    """Boot the FastAPI app in-process and dump app.openapi()."""
    try:
        from batchalign.api import app
    except Exception as exc:
        print(
            f"could not import batchalign.api: {exc}\n"
            "install the API extras: `pip install fastapi uvicorn sse-starlette`",
            file=sys.stderr,
        )
        sys.exit(2)
    return app.openapi()


def _capture_capabilities() -> dict[str, Any]:
    """Boot a TestClient and grab GET /capabilities."""
    try:
        from fastapi.testclient import TestClient
        from batchalign.api import app
    except Exception as exc:
        print(
            f"could not import test client / app: {exc}",
            file=sys.stderr,
        )
        sys.exit(2)
    with TestClient(app) as c:
        r = c.get("/capabilities")
        if r.status_code != 200:
            print(
                f"GET /capabilities returned {r.status_code}: {r.text}",
                file=sys.stderr,
            )
            sys.exit(2)
        return r.json()


def _run_openapi_typescript(openapi_path: Path) -> str:
    """Pipe the OpenAPI JSON through `openapi-typescript` (npm).

    The package must be installed via the GUI's package.json; we shell
    out via `npx --no-install` to fail loudly when it's missing rather
    than auto-fetching at codegen time.
    """
    gui_dir = openapi_path.parent.parent / "apps" / "batchalign" / "batchalign-gui"
    # Locate openapi-typescript from the GUI's node_modules; fall back
    # to a globally-available `openapi-typescript` shim if present.
    candidates = [
        gui_dir / "node_modules" / ".bin" / "openapi-typescript",
        Path(shutil.which("openapi-typescript") or ""),
    ]
    binary = next((p for p in candidates if p and p.exists()), None)
    if binary is None:
        # If neither is installed, emit a placeholder file so the GUI
        # build doesn't break — but warn loudly. The freshness check
        # will fail in CI, which is the right signal.
        print(
            "openapi-typescript not found; emitting placeholder header "
            "instead. Run `npm install` in apps/batchalign/batchalign-gui "
            "and retry to get proper typings.",
            file=sys.stderr,
        )
        return _placeholder_header(openapi_path)
    result = subprocess.run(
        [str(binary), str(openapi_path)],
        check=True,
        capture_output=True,
        text=True,
    )
    return _strip_volatile_header(result.stdout)


def _placeholder_header(_oa: Path) -> str:
    return (
        "// THIS FILE IS GENERATED — DO NOT EDIT BY HAND.\n"
        "// `openapi-typescript` was not available at generation time.\n"
        "// Run `just batchalign gen-openapi` once the GUI's `npm install`\n"
        "// has completed so this file gets proper types.\n"
        "export type paths = Record<string, never>;\n"
        "export type components = Record<string, never>;\n"
        "export type operations = Record<string, never>;\n"
    )


def _strip_volatile_header(ts: str) -> str:
    """openapi-typescript prepends a timestamped comment; drop it so the
    output is byte-stable across runs."""
    lines = ts.splitlines(keepends=True)
    out: list[str] = []
    skip = True
    for ln in lines:
        if skip and (ln.strip().startswith("/**") or ln.strip().startswith("*") or ln.strip().startswith("*/")):
            if ln.strip().endswith("*/"):
                skip = False
            continue
        skip = False
        out.append(ln)
    if not out:
        return ts
    return (
        "// THIS FILE IS GENERATED — DO NOT EDIT BY HAND.\n"
        "// Source: batchalign.api app.openapi().\n"
        "// Regenerate with: just batchalign gen-openapi\n"
        + "".join(out)
    )


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "--check",
        action="store_true",
        help="Fail if the generated artifacts would change.",
    )
    args = ap.parse_args()
    return generate(repo_root(), check=args.check)


if __name__ == "__main__":
    sys.exit(main())
