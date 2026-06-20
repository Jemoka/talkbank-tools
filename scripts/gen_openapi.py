#!/usr/bin/env python3
"""Generate OpenAPI and capabilities artifacts for the GUI.

The daemon's HTTP surface is the single source of truth (every recipe,
backend, and per-recipe Pydantic schema is derived from Python
signatures at startup; see `python/batchalign/api.py`). This script
captures that surface as Bazel outputs:

  1. `openapi.snapshot.json` — `app.openapi()`.
  2. `capabilities.snapshot.json` — captured `GET /capabilities`.

Usage:
  bazel build //apps/batchalign/batchalign-gui:protocol_artifacts
  python scripts/gen_openapi.py --out-dir /tmp/batchalign-protocol
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


def repo_root() -> Path:
    """Resolve repo root from this script's location."""
    return Path(__file__).resolve().parent.parent


def generate(
    repo: Path,
    *,
    out_dir: Path,
) -> int:
    """Generate protocol JSON artifacts into `out_dir`."""
    out_dir.mkdir(parents=True, exist_ok=True)
    snapshot_out = out_dir / "capabilities.snapshot.json"
    openapi_snapshot_out = out_dir / "openapi.snapshot.json"

    openapi_json = _capture_openapi()
    capabilities_json = _capture_capabilities()
    # `config.workdir` is host-derived (~/.cache vs $TMPDIR/...); strip
    # the absolute path before snapshotting so the file is portable.
    _normalize_capabilities(capabilities_json)
    expected_openapi_snap = json.dumps(openapi_json, indent=2, sort_keys=True) + "\n"
    expected_capabilities_snap = (
        json.dumps(capabilities_json, indent=2, sort_keys=True) + "\n"
    )

    openapi_snapshot_out.write_text(expected_openapi_snap)
    snapshot_out.write_text(expected_capabilities_snap)
    print(f"wrote {openapi_snapshot_out}")
    print(f"wrote {snapshot_out}")
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


def _normalize_capabilities(d: dict[str, Any]) -> None:
    """Strip host-specific fields from the capabilities response so the
    snapshot is portable across developers + CI.

    Currently: `config.workdir` (an absolute cache-dir path that depends
    on the runner's home directory / $TMPDIR).
    """
    cfg = d.get("config")
    if isinstance(cfg, dict):
        cfg.pop("workdir", None)


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


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "--out-dir",
        type=Path,
        default=repo_root() / "bazel-bin/apps/batchalign/batchalign-gui",
        help="Directory where generated protocol artifacts are written.",
    )
    args = ap.parse_args()
    return generate(repo_root(), out_dir=args.out_dir)


if __name__ == "__main__":
    sys.exit(main())
