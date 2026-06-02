# Contributing to talkbank-tools

**Last updated:** 2026-06-01 01:05 PDT

Welcome. This repo is a polyglot monorepo orchestrated by **Bazel**. It
ships two end-user products:

  - **batchalign** — Rust engine (`crates/batchalign/batchalign-engine`)
    surfaced as a Python wheel + `batchalign3` CLI. Pyo3 cdylib built
    natively by Bazel; release wheel built by maturin.
  - **chatter** — CHAT validation CLI (`chatter`), language server
    (`chatter-lsp`), VS Code extension, and a Tauri desktop GUI
    (`Chatter.app`).

Plus shared infrastructure: a tree-sitter grammar, a CHAT specification
+ test-generator, the `clan-core` analysis library, JSON Schemas, an
mdBook, and a fuzz workspace.

---

## Getting started

This section is the full bootstrap from a fresh machine. Skip to
[Verify your setup](#verify-your-setup) if your box is already
provisioned.

### What Bazel provides for free

The monorepo runs on **Bazel**, which fetches and pins most of its
own toolchain. After the host prerequisites below, the following
arrive hermetically the first time you `just build`:

| Tool | Version pinned in | Comes from |
|---|---|---|
| `rustc` / `cargo` | `MODULE.bazel` (`rust.toolchain`) | `rules_rust` |
| `python` (3.12) | `MODULE.bazel` (`python.toolchain`) | `rules_python` |
| `uv` | `MODULE.bazel` + `pyproject.toml` | `rules_uv` + multitool |
| `maturin` | `python/pyproject.toml` (`[build-system]`) | uv venv |
| `clang` / `llvm-ar` / `llvm-ranlib` | `MODULE.bazel` (`toolchains_llvm`) | `toolchains_llvm` |
| `mdbook` / `tree-sitter` / `vsce` / `re2c` | `multitool.lock.json` | upstream GitHub releases |
| `node` (22.12.0) | `MODULE.bazel` (`node.toolchain`) | `rules_nodejs` |
| `protoc` (29.0) | `MODULE.bazel` | `protobuf` module |

You do **not** install any of those yourself. The hermeticity guard
(`bazel/python/hermeticity_guard.sh`) asserts that pinned versions
match what's actually live before any host-tool shell-out.

### Host prerequisites

The base toolchain (Bazel build/test, batchalign sidecar, batchalign
wheel, batchalign3 CLI, chatter CLI/LSP) needs only:

  - a Bazel launcher (`bazelisk` recommended — it picks up the
    workspace's `tools/bazel` wrapper for lockfile auto-reactivity),
  - the `just` task runner,
  - git,
  - an OS-specific C toolchain (a working host `cc` — Bazel's
    `toolchains_llvm` ships its own hermetic clang on Linux/Windows
    and uses Apple's bundled clang via `xcrun` on macOS).

Nothing else is required at the host level. Bazel-managed
dependencies (cargo / rustc / cargo-tauri / pyapp / node / python /
maturin / uv / llvm / sqlite for chatter) are NOT host installs —
Bazel fetches them on first build.

**Two product groups have additional host prereqs:**

  - **chatter** (CLI, LSP, desktop GUI): links the SQLite-backed
    validation cache (`talkbank-cache`) via `libsqlite3-sys`. See
    [Chatter-only host prereqs](#chatter-only-host-prereqs).
  - **Tauri desktop apps** (`apps/chatter/chatter-gui` and
    `apps/batchalign/batchalign-gui`): link the host's WebView + UI
    frameworks; on macOS this means **full Xcode**, not just CLT. See
    [Desktop-GUI host prereqs](#desktop-gui-host-prereqs).

Skip both sections if you're only working on batchalign (Rust engine,
wheel, sidecar, `batchalign3` CLI) or the spec / grammar /
talkbank-* library crates.

#### Per-platform install one-liners

| Platform | Base toolchain |
|---|---|
| macOS | `brew install bazelisk just git` + `xcode-select --install` (CLT is sufficient unless you build Tauri apps — see below). |
| Linux (Debian/Ubuntu) | `apt-get install -y git curl build-essential pkg-config ca-certificates` plus a Bazel launcher (`npm i -g @bazel/bazelisk` or the bazel apt repo) and `just`. |
| Linux (Fedora/RHEL) | `dnf install -y git gcc gcc-c++ make pkgconf-pkg-config` plus the same Bazel launcher + `just`. |
| Windows | `scoop install bazelisk just git` plus Visual Studio 2022 Build Tools (MSVC). WSL2 + the Linux row is the easier path. |

Linux distros ship matched clang/SDK by default; the macOS Xcode caveat
applies only for the Tauri desktop apps (next section).

### Chatter-only host prereqs

The chatter CLI, LSP, and desktop GUI link the `talkbank-cache` crate
(`crates/core/talkbank-cache`), which talks to a SQLite validation
cache via `sqlx-sqlite` with the `sqlite-unbundled` feature. That
feature **links** against the system sqlite headers/library rather
than compiling a bundled `sqlite3.c`; this sidesteps the macOS-26.x
CLT SDK regressions that prevent bundled sqlite3.c from compiling.

If you're building or testing any chatter target
(`just chatter ...`, `bazel ... //crates/chatter/...`,
`bazel ... //apps/chatter/...`), install one of:

```bash
brew install sqlite                                     # macOS
sudo apt-get install -y libsqlite3-dev                  # Debian/Ubuntu
sudo dnf install -y sqlite-devel                        # Fedora/RHEL
vcpkg install sqlite3:x64-windows-static                # Windows
```

Batchalign does not need any of this — its cache is `redb`, a pure-Rust
embedded KV store.

### Desktop-GUI host prereqs

The Tauri desktop apps embed the host's WebView via Tauri, which links
against native UI libraries that aren't vendorable through Bazel.
**The base toolchain provides `cargo-tauri` itself** (`//bazel/tauri:cargo_tauri`,
SHA-pinned crates.io tarball, PATH-injected by the bundle wrapper) —
you do not need `cargo install tauri-cli`. The host prereqs below are
for Tauri's runtime linkage only.

These apply to:
  - `apps/chatter/chatter-gui`     (Chatter.app — chatter desktop)
  - `apps/batchalign/batchalign-gui` (Batchalign.app — experimental)

- **macOS:** install **full Xcode** (not just CLT), accept the license
  (`sudo xcodebuild -license accept`), and point `xcode-select` at
  `Xcode.app`. Recent macOS releases (26.x) ship a CLT bundle whose
  clang and SDK disagree, which breaks Tauri's bundler chain. Verify
  with `xcrun --sdk macosx --show-sdk-path` — it must live inside
  `Xcode.app`, not CommandLineTools.
- **Linux (Debian/Ubuntu):** `apt-get install -y libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libssl-dev patchelf nodejs npm`.
- **Linux (Fedora/RHEL):** `dnf install -y webkit2gtk4.1-devel libappindicator-gtk3-devel librsvg2-devel openssl-devel patchelf nodejs npm`.
- **Windows:** WebView2 ships with Windows 11 (Edge runtime on 10). Only Node.js is extra.

### Verify your setup

```bash
git clone https://github.com/TalkBank/talkbank-tools && cd talkbank-tools

# First build pulls every hermetic tool from multitool / rules_rust /
# rules_python; expect 5-15 minutes the very first time.
just build                     # `bazel build --config=release //...`
just test                      # `bazel test  --config=release //...`

# Smoke-test each product's CLI:
just batchalign cli -- --help
just chatter cli -- --help
```

Lockfile drift is automatic: `tools/bazel` (a Bazelisk wrapper) sees
`Cargo.toml` or `python/pyproject.toml` newer than its lockfile and
regenerates the lockfile inline before handing off to the real Bazel.
No separate command to remember. If you ever want to bypass that
behavior (debugging the wrapper itself), set
`TALKBANK_BAZEL_SKIP_RESOLVE=1`.

If `xcrun` errors with "agreed to the Xcode license" on macOS, run
`sudo xcodebuild -license accept` and retry. (Only the Tauri desktop
apps require Xcode in the first place — see
[Desktop-GUI host prereqs](#desktop-gui-host-prereqs).) If
`xcrun --sdk macosx --show-sdk-path` still points at CommandLineTools
after `xcode-select -s`, your shell may be caching `DEVELOPER_DIR` —
open a fresh terminal.

### Daily workflow

`just --list` shows every recipe. Each product has its own scope:

```bash
just --list                # workspace hub
just --list batchalign     # batchalign recipes
just --list chatter        # chatter recipes
just --list spec           # spec generators
just --list clan           # CLAN crate
just --list vscode         # VS Code extension
just --list docs           # mdBook
just --list tooling        # sqlx-prepare, xtask (lockfiles auto-regen via tools/bazel)
```

Most recipes accept a `profile` argument: `release` (default; opt
build, stripped) or `debug` (dbg build, fast incremental). They map to
Bazel's `--config=release` / `--config=dev`.

### Architecture decisions: sqlite linking

Chatter's `talkbank-cache` links against host libsqlite3 via the
`sqlx-sqlite/sqlite-unbundled` feature (sidesteps the macOS CLT 26.x
bundled-`sqlite3.c` regression). Batchalign uses `redb` instead and
needs no host sqlite. See `book/src/architecture/sqlite-linking.md`
for the full rationale and switch-back instructions.

---

## Product: batchalign

### Architecture

```
crates/batchalign/
  batchalign-core      pure Rust library (Chat, BAValue, TaskRunner, etc.)
  batchalign-engine    pyo3 bindings; produces `_core.so` cdylib

python/batchalign/     Python package; CLI (typer), backends, recipes
python/pyproject.toml  source of truth for wheel version + deps
```

The cdylib is built **natively by Bazel** via `rust_shared_library` +
macOS `-undefined dynamic_lookup` + a static
`bazel/python/pyo3-config.txt` (abi3-py312,
`suppress_build_script_link_lines=true`). `py_library`/`py_binary`/
`py_test` consume the `.so` directly. There is no `maturin develop`
loop on the dev path. The release wheel is built by maturin —
the only path that needs uv + maturin pinned by the hermeticity guard.

### Build

| Command | What it does |
|---|---|
| `just batchalign build`        | release build (Rust + cdylib + py_binary + wheel) |
| `just batchalign build debug`  | debug build (fast incremental, dbg symbols) |

### Run

| Command | What it does |
|---|---|
| `just batchalign cli -- --help`              | run the `batchalign3` CLI (Bazel-native; no maturin in path) |
| `just batchalign cli -- transcribe foo.wav`  | full CLI invocation |

### Test

| Command | What it does |
|---|---|
| `just batchalign test`                  | release-build test (Rust unit tests + py_test pytest) |
| `just batchalign test debug`            | debug-build test (faster iteration on failures) |
| `just batchalign pytest`                | only the py_test pytest suite |
| `just batchalign pytest -- -k whisper`  | pytest with extra args |
| `just batchalign lint`                  | mypy (+ ruff if present) |

### Debug

```bash
just batchalign build debug                          # dbg build of everything
just batchalign cli -- <subcommand> --verbose        # rebuilds + runs dbg .so
just batchalign pytest -- -k <name> --pdb            # pytest under pdb
# Cargo escape hatch for Rust-side debugging at higher fidelity:
cargo nextest run -p batchalign-engine --features extension-module
```

### Edit-and-rebuild reactivity

Any `.rs` edit in the engine's transitive closure (batchalign-engine,
batchalign-core, talkbank-model, talkbank-parser, talkbank-transform)
invalidates `_core.so` automatically. Re-running `just batchalign cli`
rebuilds the .so and the py_binary launcher in one shot — no
`maturin develop` step.

Editing `python/pyproject.toml` (adding a dep, bumping the version) is
also automatic: `tools/bazel` detects pyproject is newer than
`python/requirements.lock.txt` and regenerates both
`requirements.lock.txt` (Bazel-consumed) and `python/uv.lock`
(maturin-consumed) before handing off to Bazel. The next
`just batchalign cli` / `bazel run //python/batchalign` "just works"
against the refreshed deps. Same story for editing any `Cargo.toml` —
crate_universe re-resolves on the next Bazel invocation.

### Release a wheel

`python/pyproject.toml [project].version` is the source of truth for
the wheel version. `just batchalign wheel` builds a host-platform
wheel into `python/target/wheels/`. Cross-platform wheels and PyPI
uploads are CI-only — see `book/src/operations/release-pipeline.md`
for the full release flow.

### Troubleshooting: macOS SDK header errors

If a cc-rs-driven crate fails with `__kernel_ptr_semantics` /
`__sized_by` / `fixpt_t` / `__deprecated_enum_msg` parse errors against
headers under `mach/`, `sys/sysctl.h`, or `sys/event.h`, you're hitting
the CommandLineTools 26.x mismatched-clang/SDK bug. Install full Xcode
and select it (see [Desktop-GUI host prereqs](#desktop-gui-host-prereqs));
verify with `xcrun --sdk macosx --show-sdk-path`. Linux + Windows are
unaffected.

### Hermeticity pins

The maturin/wheel path uses host-side tools (cargo, host SDK) that sit
outside Bazel's sandbox. `bazel/python/hermeticity_guard.sh` asserts
uv/maturin/python/rustc versions match the pins in
`python/pyproject.toml [tool.batchalign.pinned_tools]` and scrubs
leak-prone env vars before any shell-out. Bumping a tool means updating
`pyproject.toml`, `MODULE.bazel`, AND `rust-toolchain.toml` in the same
commit.

For the hermetic C toolchain (`toolchains_llvm`), macOS SDK fallout, and
the Bazel-driven profile selection contract (no `MATURIN_PROFILE`
rituals), see `book/src/architecture/hermeticity.md` and the auto-memory
note `macos-xcode-hermeticity-gap`.

---

## Product: chatter

### Architecture

```
crates/chatter/
  chatter-cli          `chatter` binary (validate, normalize, to-json, clan ...)
  chatter-lsp          `chatter-lsp` Language Server Protocol server

apps/chatter/
  chatter-gui          Tauri v2 desktop app (Chatter.app); React + TS frontend +
                       Rust backend linking talkbank-* + send2clan-sys

apps/vscode-extension  VS Code marketplace extension (TS; talks to chatter-lsp)
```

The CLI + LSP + desktop GUI share the same Rust foundation
(`talkbank-model`, `talkbank-parser`, `talkbank-transform`,
`clan-core`, `send2clan-sys`).

### Build

| Command | What it does |
|---|---|
| `just chatter build`           | release build of CLI + LSP + Tauri-linked crates |
| `just chatter build debug`     | debug build |

The Tauri GUI itself is built by `cargo tauri build` (not Bazel —
Tauri's bundling chain includes codesign/notarytool/signtool which
aren't modelled in Bazel rules). `just chatter gui` wraps it; Bazel
still builds the intra-workspace Rust crates the GUI links against.

### Run

| Command | What it does |
|---|---|
| `just chatter cli -- validate file.cha`    | validation |
| `just chatter cli -- to-json file.cha`     | parse + emit JSON |
| `just chatter cli -- clan freq file.cha`   | CLAN frequency analysis |
| `just chatter lsp`                         | LSP server (stdin/stdout) |
| `just chatter gui`                         | bundle the desktop app (release) |
| `just chatter gui debug`                   | bundle in debug mode |
| `cd apps/chatter/chatter-gui && cargo tauri dev` | dev mode with hot reload (Tauri escape hatch) |

### Test

| Command | What it does |
|---|---|
| `just chatter test`         | release-build test (CLI + LSP unit tests) |
| `just chatter test debug`   | debug-build test |
| `cargo nextest run -p chatter-cli` | Cargo escape hatch for filtering individual tests |

### Debug

```bash
just chatter build debug
just chatter cli -- validate file.cha            # rebuilds + runs dbg
# or under a debugger:
lldb -- bazel-bin/crates/chatter/chatter-cli/chatter validate file.cha
```

For the LSP, point your editor's client at
`bazel-bin/crates/chatter/chatter-lsp/chatter-lsp` after `just chatter
build debug`. LSP logs to stderr; capture via your client's log redirect.

For the Tauri GUI, dev mode with hot reload is the fast loop:

```bash
cd apps/chatter/chatter-gui
cargo tauri dev
```

Frontend React DevTools work inside the webview; backend logs go to
stderr (visible in the terminal that spawned `cargo tauri dev`).

### Release

CLI + LSP ship as platform binaries on GitHub Releases; the GUI ships
as signed/notarized `.app`/`.msi`/`.AppImage` bundles. Until the
`publish-chatter.yml` / `publish-desktop.yml` / `publish-vscode.yml`
workflows land, releases are manual via `just chatter build`,
`just chatter gui`, and `just vscode package`. Signing/notarization
steps live in `book/src/operations/code-signing-and-distribution.md`.

### Versioning

Chatter's Rust artifacts inherit the workspace version
(`Cargo.toml [workspace.package].version`). The desktop bundle has its
own version pin in `apps/chatter/chatter-gui/src-tauri/Cargo.toml` and
`tauri.conf.json` (they must match). Print all source-of-truth
versions:

```bash
just versions
```

---

## Library crates (publishing to crates.io)

The repo holds these as library crates that may eventually publish to
crates.io:

  - `talkbank-model`, `talkbank-derive`, `talkbank-parser`,
    `talkbank-transform`, `talkbank-parser-re2c` — the CHAT data model
    + parsing stack
  - `clan-core` — CLAN analysis library
  - `chatter-lsp` — LSP server (can be consumed as a crate)
  - `batchalign-core` — batchalign's pure-Rust core (TaskRunner traits
    etc.)

None are published today. The expected workflow when you want to:

```bash
# from the workspace root
cargo publish -p talkbank-model              # publishes that one crate
# or, in dep order, the whole stack:
cargo publish -p talkbank-derive
cargo publish -p talkbank-model
cargo publish -p talkbank-parser-re2c
cargo publish -p talkbank-parser
cargo publish -p talkbank-transform
cargo publish -p clan-core
cargo publish -p chatter-lsp
cargo publish -p batchalign-core
```

Each crate's `Cargo.toml` must declare `[package] description`,
`license`, `repository`, `readme`, etc., or `cargo publish` will
refuse. The workspace's `Cargo.toml [workspace.package]` inherits most
of these; check that any individual `[package]` overrides don't omit
required fields.

Bazel does not run `cargo publish`. Use a personal crates.io token (or
a CI secret) when running these locally.

To wire a CI workflow for crates.io: copy the structure of
`publish-pypi.yml`, swap maturin/twine for `cargo publish`, and gate
on a tag like `crates-v<x.y.z>`. Not done yet.

---

## How this repo is built (overview)

Bazel is the single entry point; it orchestrates each ecosystem's
canonical tooling (`rules_rust` + `crate_universe`, `rules_python` +
`rules_uv`, maturin for release wheels, `cargo tauri build` for Tauri
bundles, `npm`/`vsce` for the VS Code extension, `tree-sitter generate`
for the grammar, `mdbook` for the book). Cargo still works at the
workspace root as an escape hatch.

For the full surface-by-surface table and the canonical `bazel run`
target list (chatter, batchalign, spec generators, docs, VS Code,
grammar, dev tooling), see `book/src/contributing/bazel-workflows.md`.

---

## Common workflows

### "I edited a Cargo.toml"

Nothing. `tools/bazel` regenerates `Cargo.lock` and the crate_universe
metadata on the next Bazel invocation. Just run whatever command you
were going to run (`just build`, `bazel test //...`, etc.).

### "I edited a `sqlx::query!` in batchalign"

```bash
just tooling sqlx-prepare                                 # = bazel run //bazel/sqlx:prepare
```

Commit the resulting `crates/batchalign/.../.sqlx/` directory.

### "I edited grammar.js"

```bash
just spec gen-tree-sitter-tests                           # regenerates corpus tests + parser.c
just test                                                 # workspace-wide test (includes parser)
```

### "I edited a spec under resources/spec/"

```bash
just spec gen-tree-sitter-tests
just spec gen-rust-tests
just spec gen-validation-tests
just spec gen-error-docs
just spec validate
```

### "I edited Python in python/batchalign/"

```bash
just batchalign cli -- --help                             # smoke: rebuilds .so, runs CLI
just batchalign pytest                                    # full test suite
just batchalign lint                                      # mypy + ruff
```

No `maturin develop` step — `just batchalign cli` rebuilds the `.so`
natively via the rust_shared_library → genrule → py_library chain.

### "I edited pyproject.toml"

Nothing. `tools/bazel` regenerates `python/requirements.lock.txt` (and
`python/uv.lock` if `uv` is on PATH) on the next Bazel invocation.
Commit `pyproject.toml` and both lockfiles together when you're ready
to open the PR — `git status` will surface them after the next build.

### "I want a release wheel"

Local development only ever produces a host-platform wheel:

```bash
just batchalign wheel
ls python/target/wheels/
```

For a real release, dispatch `publish-pypi.yml` from the Actions UI —
it fans out to one runner per `(platform, arch)` cell, produces every
native triple's wheel, and uploads via PyPI's OIDC trusted-publisher
flow. There is no local publish path.

---

## GitHub Actions (namespace-scoped)

`.github/workflows/` is split by surface, path-filtered so each workflow
only runs against changes affecting it (Rust, Python, wheels,
TypeScript, grammar, docs, the nightly `bazel build //... && bazel test
//...` job, and the manual PyPI publish). For the surface-by-surface
table, see `book/src/operations/release-pipeline.md`.

---

## Repository layout

```
crates/
  core/      talkbank-{model, derive, parser, transform, parser-re2c, parser-tests}
  chatter/   chatter-{cli, lsp}                  → produces `chatter` + `chatter-lsp`
  clan/      clan-core, send2clan-sys            → CLAN analysis + macOS FFI shim
  batchalign/  batchalign-{core, engine}         → pyo3 engine for the wheel
  spec/      talkbank-spec-{testgen, testrun}    → spec → test generators
  xtask/     workspace dev automation
  utils/     workspace utilities
apps/
  chatter/         chatter-gui                    (Tauri v2 desktop app)
  vscode-extension                                (TypeScript)
python/      pyproject.toml + requirements.lock.txt; batchalign/ + batchalign_core/
grammar/     tree-sitter CHAT grammar (multi-language bindings)
resources/   corpus/ (sacred), fixtures/, spec/ (source of truth)
schemas/     chat-file/, ipc/ (JSON Schema)
book/        mdBook documentation
bazel/       Bazel-internal shell wrappers + pyo3-config + hermeticity guard
fuzz/        cargo-fuzz workspace (separate from root)
```

Tier-by-tier deep dives:

- `crates/*/<crate>/CLAUDE.md` — every crate has a local guide
- `apps/*/CLAUDE.md` — per-app architectural rules
- `python/batchalign/README.md` — Python package overview
- `grammar/CLAUDE.md` — grammar change workflow
- `resources/spec/CLAUDE.md` — spec system overview
- `book/src/operations/release-pipeline.md` — full chain from edit → PyPI / Marketplace / GitHub Release

> Note: the `crates/batchalign/tests/ml_golden/` regression-runner referenced
> by some fixture READMEs and design docs is aspirational; it is being staged
> on a private branch and is not yet present in the public tree.

---

## Coding standards (cross-cutting)

- **Rust:** edition 2024, `cargo fmt`, `cargo clippy --all-targets`, no panic-in-control-flow.
- **TypeScript:** strict mode on; lint with project `tsconfig.json` defaults.
- **Python:** mypy strict (see `python/mypy.ini`); pytest in `python/pytest.ini`.
- **Comments:** explain WHY, never WHAT. Don't write commit/PR-context into source files.
- **CLAUDE.md files:** if you touch any documentation file, update its `Last modified:` stamp. Run `date '+%Y-%m-%d %H:%M %Z'` for the actual time.
- **Specs are source of truth.** Never hand-edit generated artifacts under `grammar/test/corpus/`, `crates/.../tests/generated/`, or `book/src/operations/errors/`.

The book has the full coding-standards chapter at
`book/src/contributing/coding-standards.md`.

---

## Get help

- Open an issue on GitHub
- Read the book first: `just docs serve`
- For Bazel-specific gotchas: `book/src/contributing/bazel-workflows.md`
- For the release chain: `book/src/operations/release-pipeline.md`
