# Contributing to talkbank-tools

**Last updated:** 2026-05-15 20:24 IST

Welcome. This repo is a polyglot monorepo: Rust workspace + Python (PyO3
via maturin) + TypeScript (VS Code extension + React dashboard) +
tree-sitter grammar + Tauri desktop apps + mdBook documentation. All of
it is orchestrated by **Bazel**.

---

## Quick start

```bash
# Prerequisites (install once)
# - bazelisk (will auto-fetch the Bazel version pinned in .bazelversion)
brew install bazelisk          # macOS
# or: npm install -g @bazel/bazelisk

# Everything else (uv, mdbook, tree-sitter) is fetched hermetically by
# Bazel from `multitool.lock.json` the first time you build.

# Clone + build everything you need to run anything
git clone https://github.com/TalkBank/talkbank-tools && cd talkbank-tools
bazel build //...               # Compile every Rust crate, dashboard, book
bazel test  //...               # Run every Rust unit test
```

If the first build complains about `crate_universe`, run:

```bash
bazel run //bazel/cargo:repin   # syncs MODULE.bazel.lock with Cargo.lock
```

That's it. Every other workflow is below.

---

## How this repo is built

**Bazel is the single entry point.** It orchestrates each ecosystem's
canonical tooling rather than replacing it:

| Surface | Tool Bazel calls | Why |
|---|---|---|
| Rust workspace | `rules_rust` + `crate_universe` | Bazel handles deps + caching natively. |
| Python wheel (PyO3) | `maturin` via shell wrapper | No Bazel ruleset packages PyO3 wheels (manylinux, abi3, universal2). Maturin is canonical. |
| React dashboard | `vite` via shell wrapper | Vite plugins (openapi-typescript codegen, tailwind) aren't modelled in Bazel. |
| VS Code extension | `npm` + `vsce` via shell wrappers | `vsce` is npm-only. |
| Tauri desktop bundles | `cargo tauri build` via shell wrapper | Tauri's bundling chain (codesign, notarytool, signtool) isn't modelled in Bazel. |
| tree-sitter grammar | `tree-sitter generate` via shell wrapper | Multi-language bindings (JS/Python/Go/Swift/Rust); only the Rust binding compiles through `cargo_build_script`. |
| mdBook | `mdbook` via shell wrapper | Hermetic `mdbook` binary fetched via multitool. |

The Cargo workspace at the repo root still works (`cargo build`,
`cargo nextest run`, `cargo run -p chatter-cli -- ...`) — Bazel does not
disable Cargo; it adds a hermetic, cache-friendly path on top. Bazel is
canonical; Cargo is the escape hatch.

---

## `bazel run` reference for every major binary

### CHAT validation / LSP (Rust)

```bash
bazel run //crates/chatter/chatter-cli:chatter           # `chatter` CLI
bazel run //crates/chatter/chatter-lsp:chatter-lsp        # Language Server
```

Example: `bazel run //crates/chatter/chatter-cli:chatter -- validate
path/to/file.cha`

### Batchalign (Rust + Python)

```bash
bazel run //crates/batchalign/batchalign-cli:batchalign3   # batchalign3 server/CLI
bazel run //python/batchalign:develop                      # editable install (maturin develop)
bazel run //python/batchalign:wheel                        # build distributable wheel
bazel run //python/batchalign:test                         # pytest
bazel run //python/batchalign:lint                         # mypy (+ ruff)
```

### CLAN spec generators

```bash
bazel run //crates/spec/talkbank-spec-testgen:gen_tree_sitter_tests
bazel run //crates/spec/talkbank-spec-testgen:gen_rust_tests
bazel run //crates/spec/talkbank-spec-testgen:gen_validation_tests
bazel run //crates/spec/talkbank-spec-testgen:gen_error_docs
bazel run //crates/spec/talkbank-spec-testgen:validate_spec
bazel run //crates/spec/talkbank-spec-testgen:coverage
bazel run //crates/spec/talkbank-spec-testrun:validate_error_specs
bazel run //crates/spec/talkbank-spec-testrun:extract_corpus_candidates
```

### Workspace dev tooling

```bash
bazel run //crates/xtask:xtask -- <subcommand>
bazel run //bazel/cargo:repin                       # after any Cargo.toml edit
bazel run //bazel/sqlx:prepare                      # after any sqlx::query! edit
```

### Docs

```bash
bazel run //book:serve         # preview at http://localhost:3000
bazel run //book:html          # static HTML at book/build/html/
bazel run //book:linkcheck     # mdbook build + linkcheck preprocessor
```

### Apps

```bash
bazel run //apps/vscode-extension:build
bazel run //apps/vscode-extension:package           # produces .vsix
bazel run //apps/vscode-extension:test
bazel build //apps/batchalign/batchalign-cli-webdashboard:dist  # hermetic vite → dist/ TreeArtifact
```

Tauri desktop bundles are produced by the `publish-desktop` workflow
because the bundling chain needs platform signing certificates. Locally:
`cd apps/<...>/<gui>/ && cargo tauri dev`.

### Tree-sitter grammar (Rust binding only via Bazel)

```bash
bazel build //grammar:tree_sitter_talkbank
bazel test  //grammar:tree_sitter_talkbank_unit_test
```

Grammar regeneration (`tree-sitter generate`) runs against
`@multitool//tools/tree-sitter`. See `book/src/contributing/grammar.md`
for the workflow.

---

## Tests

```bash
bazel test //...                                  # every Rust unit test
bazel test //crates/core/...                      # core tier only
bazel test //crates/chatter/...                   # chatter tier only
bazel test //crates/batchalign/...                # batchalign tier only
bazel test //crates/clan/...                      # CLAN tier only
bazel run  //python/batchalign:test                       # Python tests
bazel run  //apps/vscode-extension:test           # VS Code extension tests
```

Integration tests (workspace-level `talkbank-utils/tests/`, batchalign's
`tests/`, the React dashboard's Playwright E2E) are not yet wired into
`bazel test`. They run via the GitHub workflows for now.

---

## Common workflows

### "I edited a Cargo.toml"

```bash
bazel run //bazel/cargo:repin
```

### "I edited a `sqlx::query!` in batchalign"

```bash
bazel run //bazel/sqlx:prepare
```

Commit the resulting `crates/batchalign/batchalign-cli/.sqlx/` directory.

### "I edited grammar.js"

```bash
bazel run //grammar:tree_sitter_generate          # regenerates src/parser.c
bazel build //grammar:tree_sitter_talkbank
bazel test  //crates/core/talkbank-parser:talkbank_parser_unit_test
```

(Tree-sitter generate target wraps `@multitool//tools/tree-sitter`.)

### "I edited a spec under resources/spec/"

```bash
bazel run //crates/spec/talkbank-spec-testgen:gen_tree_sitter_tests
bazel run //crates/spec/talkbank-spec-testgen:gen_rust_tests
bazel run //crates/spec/talkbank-spec-testgen:gen_validation_tests
bazel run //crates/spec/talkbank-spec-testgen:gen_error_docs
bazel test //crates/core/talkbank-parser-tests:...
```

### "I edited Python in python/batchalign/"

```bash
bazel run //python/batchalign:develop            # rebuild the .so and reinstall
bazel run //python/batchalign:test
bazel run //python/batchalign:lint
```

### "I want a release wheel"

```bash
bazel run //python/batchalign:wheel
ls python/target/wheels/                    # batchalign3-0.1.0-py3-*.whl
```

The release workflow (`.github/workflows/publish-pypi.yml`) does this
in CI across Linux + macOS x86_64 + arm64 and uploads to PyPI.

---

## GitHub Actions (namespace-scoped)

`.github/workflows/` is split by surface. Each workflow only runs on
changes affecting its surface (path filters do the gating):

| Workflow | When it runs | What it does |
|---|---|---|
| `bazel-rust.yml` | crates/, grammar/, Cargo.toml, MODULE.bazel | build + unit-test every Rust crate |
| `bazel-python.yml` | python/, batchalign-pyo3/, batchalign-types/, talkbank-transform/ | maturin develop + pytest + mypy |
| `bazel-typescript.yml` | apps/vscode-extension/, apps/batchalign/batchalign-cli-webdashboard/ | extension build + .vsix package + dashboard SPA build |
| `bazel-grammar.yml` | grammar/, resources/spec/symbols/ | Rust binding + regen-drift check |
| `bazel-docs.yml` | book/ | mdbook build + linkcheck |
| `bazel-build-all.yml` | cron (06:00 UTC) + manual | `bazel build //...` + `bazel test //...` |
| `publish-pypi.yml` | manual | batchalign3 wheel → PyPI (OIDC trusted-publisher) |
| `publish-chatter.yml` | manual | chatter + chatter-lsp binaries → GitHub Release |
| `publish-vscode.yml` | manual | .vsix → VS Code Marketplace |
| `publish-desktop.yml` | manual | signed Tauri bundles → GitHub Release |

All workflows use `bazel-contrib/setup-bazel@0.15.0` for Bazel + cache.

---

## Repository layout

```
crates/
  core/      talkbank-{model, derive, parser, transform, parser-re2c,
                       parser-tests, spec-testgen, spec-testrun, utils}
  chatter/   chatter-{cli, lsp}                  → produces `chatter` + `chatter-lsp`
  clan/      clan-core, send2clan-sys            → CLAN analysis + macOS FFI shim
  batchalign/  batchalign-{cli, types, pyo3}     → produces `batchalign3`
  xtask/     workspace dev automation
apps/
  chatter/      chatter-gui                       (Tauri)
  batchalign/   batchalign-cli-webdashboard       (React+Vite)
                batchalign-gui-dashboard          (Tauri)
  vscode-extension                                (TypeScript)
python/      pyproject.toml + uv.lock; batchalign/, batchalign_core/
grammar/     tree-sitter CHAT grammar (multi-language bindings)
resources/   corpus/ (sacred), fixtures/, spec/ (source of truth)
schemas/     chat-file/, ipc/ (JSON Schema)
book/        mdBook documentation
build/       Bazel-internal shell wrappers (uv, mdbook, vite, vsce, sqlx, …)
fuzz/        cargo-fuzz workspace (separate from root)
```

Tier-by-tier deep dives:

- `crates/*/<crate>/CLAUDE.md` — every crate has a local guide
- `apps/*/CLAUDE.md` — per-app architectural rules
- `python/batchalign/README.md` — Python package overview
- `grammar/CLAUDE.md` — grammar change workflow
- `resources/spec/CLAUDE.md` — spec system overview
- `book/src/operations/release-pipeline.md` — full chain from edit → PyPI / Marketplace / GitHub Release

---

## Coding standards (cross-cutting)

- **Rust:** edition 2024, `cargo fmt`, `cargo clippy --all-targets`, no panic-in-control-flow (see `[lints.clippy]` table in each crate's Cargo.toml; the panic-site audit notes live under `book/src/operations/panic-audit/`).
- **TypeScript:** strict mode on; lint with the project's `tsconfig.json` defaults.
- **Python:** mypy strict (see `python/mypy.ini`); pytest in `python/pytest.ini`.
- **Comments:** explain WHY, never WHAT. Don't write commit/PR-context into source files.
- **CLAUDE.md files:** if you touch any documentation file, update its `Last modified:` stamp. Run `date '+%Y-%m-%d %H:%M %Z'` for the actual time.
- **Specs are source of truth.** Never hand-edit generated artifacts under `grammar/test/corpus/`, `crates/.../tests/generated/`, or `book/src/operations/errors/`.

The book has the full coding-standards chapter at
`book/src/contributing/coding-standards.md`.

---

## Get help

- Open an issue on GitHub
- Read the book first: `bazel run //book:serve`
- For Bazel-specific gotchas: `book/src/contributing/bazel-workflows.md`
- For the release chain: `book/src/operations/release-pipeline.md`
