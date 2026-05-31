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
  just batchalign::gui openapi             # run via Bazel
  python scripts/gen_openapi.py            # direct invocation (dev)

Exits non-zero if the generated artifacts would change without
committing — pair with `bazel test //apps/batchalign/batchalign-gui:
openapi_freshness` for CI freshness gating.
"""

from __future__ import annotations

import argparse
import json
import os
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
    """Generate `openapi.gen.ts` + `capabilities.snapshot.json` (+ a
    commited `openapi.snapshot.json` source-of-truth for the .ts file).

    When `check=True`, exit non-zero if the on-disk JSON snapshots
    drifted from the live FastAPI surface. The `.ts` file is treated
    as a downstream codegen artifact regenerated via `bazel run
    //apps/batchalign/batchalign-gui:openapi`, and it's checked locally
    only when `openapi-typescript` is reachable (node + node_modules);
    in the bazel-sandboxed test run the `.ts` invocation is skipped
    cleanly so the test is hermetic.
    """
    out_dir = repo / "apps/batchalign/batchalign-gui/src/protocol"
    out_dir.mkdir(parents=True, exist_ok=True)
    ts_out = out_dir / "openapi.gen.ts"
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

    if check:
        # Hermetic JSON-only drift gate. `openapi.snapshot.json` is the
        # source-of-truth for the codegen; if THAT didn't drift, the
        # .ts file can't have either (modulo openapi-typescript
        # version bumps, which are checked separately on `bazel run`).
        existing_openapi_snap = (
            openapi_snapshot_out.read_text() if openapi_snapshot_out.exists() else ""
        )
        existing_capabilities_snap = (
            snapshot_out.read_text() if snapshot_out.exists() else ""
        )
        drift: list[str] = []
        if existing_openapi_snap != expected_openapi_snap:
            drift.append("openapi.snapshot.json")
        if existing_capabilities_snap != expected_capabilities_snap:
            drift.append("capabilities.snapshot.json")
        if drift:
            print(
                f"drift detected in {', '.join(drift)}. Run "
                f"`just batchalign::gui openapi` to regenerate and commit "
                f"the result.",
                file=sys.stderr,
            )
            # Dump a unified-diff hint so CI logs surface the actual
            # divergence (otherwise contributors have to reproduce
            # locally to see what changed).
            import difflib
            if "capabilities.snapshot.json" in drift:
                hint = "".join(
                    difflib.unified_diff(
                        existing_capabilities_snap.splitlines(keepends=True),
                        expected_capabilities_snap.splitlines(keepends=True),
                        fromfile="committed",
                        tofile="live",
                        n=2,
                    )
                )
                print(hint[:4000], file=sys.stderr)
            return 1
        return 0

    # Non-check (regen) path: write the JSON snapshots, then try to
    # regenerate the .ts file when openapi-typescript is reachable.
    openapi_snapshot_out.write_text(expected_openapi_snap)
    snapshot_out.write_text(expected_capabilities_snap)
    print(f"wrote {openapi_snapshot_out.relative_to(repo)}")
    print(f"wrote {snapshot_out.relative_to(repo)}")

    with tempfile.TemporaryDirectory() as td:
        tmp = Path(td)
        oa = tmp / "openapi.json"
        oa.write_text(json.dumps(openapi_json, indent=2, sort_keys=True))
        ts = _run_openapi_typescript(oa)
    ts_out.write_text(ts)
    print(f"wrote {ts_out.relative_to(repo)}")
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


def _resolve_node() -> str | None:
    """Find a node binary for invoking the openapi-typescript JS entrypoint.

    bazel test sandboxes strip the developer's $PATH, so a plain
    `shutil.which("node")` won't find Homebrew or nvm installs. Probe
    the most common install prefixes before giving up.
    """
    via_path = shutil.which("node")
    if via_path:
        return via_path
    for candidate in (
        "/opt/homebrew/bin/node",
        "/opt/homebrew/opt/node@22/bin/node",
        "/opt/homebrew/opt/node@20/bin/node",
        "/usr/local/bin/node",
        "/usr/bin/node",
    ):
        if Path(candidate).is_file() and os.access(candidate, os.X_OK):
            return candidate
    return None


def _resolve_openapi_typescript() -> Path | None:
    """Find the openapi-typescript CLI script (JS, executable via node)."""
    gui_dir = repo_root() / "apps" / "batchalign" / "batchalign-gui"
    bin_dir = gui_dir / "node_modules" / ".bin"
    direct = bin_dir / "openapi-typescript"
    if direct.is_file():
        return direct
    # Some installs put the actual JS at node_modules/openapi-typescript/
    # bin/cli.js with .bin/<name> as a symlink. Probe that too.
    js_cli = gui_dir / "node_modules" / "openapi-typescript" / "bin" / "cli.js"
    if js_cli.is_file():
        return js_cli
    which = shutil.which("openapi-typescript")
    return Path(which) if which and Path(which).is_file() else None


def _run_openapi_typescript(openapi_path: Path) -> str:
    """Pipe the OpenAPI JSON through `openapi-typescript` (npm).

    The script's shebang is `#!/usr/bin/env node` — when invoked from a
    Bazel sandbox with a stripped $PATH the kernel can't find node and
    returns exit 127. Resolve both `node` and the openapi-typescript JS
    entrypoint here and invoke as `node openapi-typescript.js …`.
    """
    binary = _resolve_openapi_typescript()
    if binary is None:
        print(
            "openapi-typescript not found; emitting placeholder header "
            "instead. Run `npm ci` in apps/batchalign/batchalign-gui "
            "and retry to get proper typings.",
            file=sys.stderr,
        )
        return _placeholder_header(openapi_path)

    node = _resolve_node()
    if node is None:
        print(
            "node not found on PATH or common install prefixes. Install "
            "node ≥20 (Homebrew: `brew install node`).",
            file=sys.stderr,
        )
        return _placeholder_header(openapi_path)

    result = subprocess.run(
        [node, str(binary), str(openapi_path)],
        check=True,
        capture_output=True,
        text=True,
    )
    return _strip_volatile_header(result.stdout)


def _placeholder_header(_oa: Path) -> str:
    return (
        "// THIS FILE IS GENERATED — DO NOT EDIT BY HAND.\n"
        "// `openapi-typescript` was not available at generation time.\n"
        "// Run `just batchalign::gui openapi` once the GUI's `npm install`\n"
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
        "// Regenerate with: just batchalign::gui openapi\n"
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
