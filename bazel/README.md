# `build/` — repo-internal Bazel tooling

Wrappers, scripts, and small Bazel rules that drive the polyglot build
chain. Nothing in here is a product; everything in here is a **tool that
Bazel orchestrates**.

The pattern across this tree is the same: when a third-party tool already
knows how to do the job correctly (maturin builds wheels, vite builds the
React SPA, mdbook builds the book, `cargo tauri build` bundles desktop
apps, `vsce` packages VS Code extensions), Bazel **shells out** to it via
a `sh_binary` rule rather than re-modelling the work in Starlark. That
lets Bazel act as the workspace-wide entry point (`bazel run //python/batchalign:wheel`)
while leaving each ecosystem's canonical tooling in charge of what it
does best.

| Path | Purpose | Consumer |
|---|---|---|
| `cargo/` | `bazel run //bazel/cargo:repin` — regenerate `crate_universe` lock after any `Cargo.toml` edit | public |
| `sqlx/` | `bazel run //bazel/sqlx:prepare` — regenerate `.sqlx/` query cache for `batchalign-cli`'s `sqlx::query!` macros | public |
| `python/` | maturin orchestration: `develop`, `maturin_build`, `pytest`, `lint` | `//python` only |
| `dashboard/` | `vite build` wrapper for the React dashboard SPA | `//apps/batchalign/batchalign-cli-webdashboard` only |
| `vscode/` | `npm` + `vsce` wrappers for the VS Code extension | `//apps/vscode-extension` only |
| `book/` | mdBook wrappers (`serve`, `build`, `linkcheck`) | `//book` only |
| `chatter-tauri/` | `cargo tauri build` wrapper for the Chatter desktop app | `//apps/chatter/chatter-gui/src-tauri` only |

## When to add a new subdir

Add one only when:

1. There's a build chain that lives outside Bazel (typically because the
   relevant Bazel ruleset doesn't exist or doesn't model the tool's
   release semantics — manylinux wheel tagging, Tauri bundling, macOS
   notarization, etc.).
2. There's exactly one Bazel package that consumes it.
3. The native tool itself is conventionally invoked from the repo root.

If a script is genuinely workspace-wide (formatters, doc-sync helpers
that touch many packages), it belongs under `scripts/`, not here.

## Visibility convention

Every subpackage scopes `default_visibility` to the single Bazel package
that consumes it (`//visibility:private` would be even tighter, but the
`exports_files()` declarations need at least the consumer's package to
see them). The two exceptions (`cargo/` and `sqlx/`) are public because
every developer needs `repin` after a `Cargo.toml` edit and every
developer touching SQL needs `prepare`.

## Why `build/` and not `tools/`?

Bazel conventionally uses either `tools/` or `build_defs/`. We use
`build/` because the contents are unambiguously **build-related** — they
exist to drive `bazel build` / `bazel run`, never to ship as a product.
The name `tools/` invited the question "are these shipped tools?"; the
name `build/` is honest about what they are.

## Conventions inside the scripts

- Every shell script starts with `set -euo pipefail`.
- Scripts cd into `$BUILD_WORKSPACE_DIRECTORY` first so paths are
  workspace-relative regardless of where `bazel run` was invoked from.
- Scripts assume their host tool (`uv`, `maturin`, `mdbook`, `npm`,
  `vsce`, `cargo`, `sqlx-cli`, `re2c`) is on `$PATH`. CI installs them
  via setup actions; local developers install once.
- Scripts fail loudly with a readable error if the host tool is missing.
- No script writes outside the workspace.

## See also

- `CONTRIBUTING.md` (repo root) — quick start, common workflows, `bazel
  run` examples for every major binary
- `book/src/operations/release-pipeline.md` — full chain from dev → CI
  → published artifacts (PyPI, crates.io, Marketplace, GitHub Releases)
- `MODULE.bazel` — module-level dep declarations
