# `bazel/` — repo-internal Bazel tooling

Wrappers, scripts, and small Bazel rules that drive the polyglot build
chain. Nothing in here is a product; everything in here is a **tool that
Bazel orchestrates**.

Layout follows the dependency graph of the build itself: deps preparation
first, then language toolchains, then frontends, then documentation.

The pattern across this tree is the same: when a third-party tool already
knows how to do the job correctly (maturin builds wheels, vite builds the
React SPA, mdbook builds the book, `cargo tauri build` bundles desktop
apps, `vsce` packages VS Code extensions), Bazel **shells out** to it via
a `sh_binary` rule rather than re-modelling the work in Starlark. That
lets Bazel act as the workspace-wide entry point (`bazel run //python/batchalign:wheel`)
while leaving each ecosystem's canonical tooling in charge of what it
does best.

Ordered by where each entry sits in the build graph:

| Stage | Path | Purpose | Consumer |
|---|---|---|---|
| 1. Dep prep | `cargo/` | `crate_universe` repin target. **Not called directly** — `tools/bazel` triggers a repin on every Bazel invocation when `Cargo.toml` is newer than `Cargo.lock`. The target remains as a manual escape hatch. | public |
| 1. Dep prep | `sqlx/` | `bazel run //bazel/sqlx:prepare` — regenerate `.sqlx/` query cache for `batchalign-cli`'s `sqlx::query!` macros | public |
| 1. Dep prep | `patches/` | crate_universe patch files (see `MODULE.bazel`) | crate_universe only |
| 2. Toolchains | `python/` | `uv` + `maturin` orchestration: `cli`, `develop`, `maturin_build`, `pytest`, `lint`. Profile via `MATURIN_PROFILE` or `BAZEL_COMPILATION_MODE`. | `//python` only |
| 3. App bundlers | `chatter-tauri/` | `cargo tauri build` wrapper for Chatter desktop. Profile via `TAURI_PROFILE` or `BAZEL_COMPILATION_MODE`. | `//apps/chatter/chatter-gui/src-tauri` only |
| 3. App bundlers | `batchalign-tauri/` | `cargo tauri build` wrapper for Batchalign desktop. | `//apps/batchalign/batchalign-gui` only |
| 3. App bundlers | `tauri/` | shared Tauri CLI install rule (`cargo_tauri`). | both Tauri bundlers |
| 3. App bundlers | `vscode/` | `npm` + `vsce` wrappers for the VS Code extension | `//apps/vscode-extension` only |
| 4. Docs | `book/` | mdBook wrappers (`serve`, `build`, `linkcheck`) | `//book` only |

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
see them). Two exceptions are public: `cargo/` (escape-hatch repin
target; the routine flow is automatic via `tools/bazel`) and `sqlx/`
(`prepare` invoked by every developer touching SQL).

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
