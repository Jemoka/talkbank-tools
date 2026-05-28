# Contributing to talkbank-tools

**Last updated:** 2026-05-28 11:41 PDT

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

What you *do* install on the host: a Bazel launcher, the `just` task
runner, git, and an OS-specific C toolchain + system sqlite (some
`*-sys` Rust crates link, not compile, against system libraries).

#### macOS

```bash
# 1. Install Xcode (NOT just `xcode-select --install`).
#    Recent macOS releases (26.x) ship a CommandLineTools bundle whose
#    clang and SDK don't agree with each other -- every cc-rs-driven
#    build fails with `__kernel_ptr_semantics` / `__sized_by` /
#    `fixpt_t` parse errors. Full Xcode bundles a matched clang/SDK
#    pair.
mas install 497799835                                   # `brew install mas` first
# or download Xcode from https://developer.apple.com/xcode/

# 2. Accept the Xcode license + finish post-install.
sudo xcodebuild -license accept

# 3. Point xcode-select at Xcode.app, not CommandLineTools.
sudo xcode-select -s /Applications/Xcode.app/Contents/Developer

# 4. Verify the SDK path lives inside Xcode.app.
xcrun --sdk macosx --show-sdk-path
# expect: /Applications/Xcode.app/Contents/Developer/Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk
# NOT:    /Library/Developer/CommandLineTools/SDKs/MacOSX.sdk

# 5. Workspace tools.
brew install bazelisk just sqlite git

# 6. Optional: faster development feedback for the GUI / VS Code
#    extension. These are NOT required for `just build` / `just test`.
brew install node                                       # only if you want host-side `npm`
```

**Why system sqlite?** `talkbank-transform` links against system
sqlite via the `sqlx-sqlite/unbundled` feature. Bundling sqlite (the
default in many `*-sys` crates) tries to compile `sqlite3.c` against
the macOS SDK from a plain cargo invocation, which the SDK's
`<sys/sysctl.h>` etc. don't cleanly tolerate. The maturin shell
scripts (`bazel/python/{maturin,pyapp}_build.sh`) auto-detect
homebrew sqlite and export `SQLITE3_LIB_DIR` /
`SQLITE3_INCLUDE_DIR` so libsqlite3-sys's `build_linked` path picks
it up. See [Architecture decisions: sqlite linking](#architecture-decisions-sqlite-linking)
below.

**Why full Xcode and not CLT?** Bazel-native builds work fine with
either, because `rules_rust` wraps `cargo_build_script` in a
hermetic-cc shell that papers over a lot of SDK quirks. The
maturin escape path (wheel, sidecar) uses host cargo directly and
sees the raw SDK headers; with CLT 26.x those don't parse. We don't
vendor an SDK as a workaround because Apple's EULA prohibits
redistribution.

#### Linux (Debian/Ubuntu)

```bash
# Workspace tools.
sudo apt-get update
sudo apt-get install -y \
    git curl build-essential pkg-config \
    libsqlite3-dev \
    ca-certificates

# Bazel launcher + just.
# Option A (apt):
curl -fsSL https://bazel.build/bazel-release.pub.gpg | sudo gpg --dearmor -o /usr/share/keyrings/bazel-archive-keyring.gpg
echo "deb [arch=amd64 signed-by=/usr/share/keyrings/bazel-archive-keyring.gpg] https://storage.googleapis.com/bazel-apt stable jdk1.8" | sudo tee /etc/apt/sources.list.d/bazel.list
sudo apt-get update && sudo apt-get install -y bazel
cargo install just                                      # needs rust; or use the prebuilt binary

# Option B (mise / asdf / npm one-liner):
npm install -g @bazel/bazelisk
# (then `cargo install just` or download the binary from
# https://github.com/casey/just/releases)
```

Linux distros ship matched clang/SDK by default; no equivalent of the
macOS gotcha. `libsqlite3-dev` provides the headers + library the
unbundled sqlx-sqlite needs.

#### Linux (Fedora/RHEL)

```bash
sudo dnf install -y git gcc gcc-c++ make pkgconf-pkg-config sqlite-devel
# Bazel + just same as Debian (apt → dnf for the bazelisk package).
```

#### Windows

Not tested by maintainers as a daily-driver dev environment, but CI
builds wheels on Windows runners. Bare minimum:

```powershell
# winget or scoop are fine.
scoop install bazelisk just git
# Visual Studio 2022 Build Tools provides MSVC (the `*-sys` crate
# linker target on Windows).
# vcpkg for sqlite:
vcpkg install sqlite3:x64-windows-static
$env:SQLITE3_LIB_DIR = "$(vcpkg list --x-install-root .\vcpkg_installed)\x64-windows-static\lib"
$env:SQLITE3_INCLUDE_DIR = "$(vcpkg list --x-install-root .\vcpkg_installed)\x64-windows-static\include"
```

WSL2 (Ubuntu) is the easier path if you don't have a hard Windows
requirement -- the Linux instructions above work as-is.

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

If the first build complains about `crate_universe`, run
`just tooling cargo-repin` (which is `bazel run //bazel/cargo:repin`).

If `xcrun` errors with "agreed to the Xcode license" on macOS, run
`sudo xcodebuild -license accept` and retry. If `xcrun --sdk macosx
--show-sdk-path` still points at CommandLineTools after step 3, your
shell may be caching `DEVELOPER_DIR` -- open a fresh terminal.

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
just --list tooling        # cargo-repin, sqlx-prepare, xtask
```

Most recipes accept a `profile` argument: `release` (default; opt
build, stripped) or `debug` (dbg build, fast incremental). They map to
Bazel's `--config=release` / `--config=dev`.

### Architecture decisions: sqlite linking

`talkbank-transform`'s public API includes `UnifiedCache`, a
`sqlx::SqlitePool`-backed validation/roundtrip cache. It's consumed by
**chatter** (`chatter-cli`'s `cache clear`, `validate`; the test
dashboard; roundtrip-corpus regression tests) and is NOT used by
**batchalign-engine** (which has its own `redb`-backed cache in
`crates/batchalign/batchalign-engine/src/cache.rs`).

Because `talkbank-transform` is a shared dep, the batchalign wheel
transitively links sqlite even though Batchalign code never touches
it. We accept the unused-link tax rather than splitting the cache
into its own crate -- the diff isn't worth the maintenance cost. If
that calculus changes, the right refactor is to move `unified_cache`
into a separate `talkbank-cache` crate that only chatter depends on,
or to feature-gate it behind `#[cfg(feature = "sqlite-cache")]` on
`talkbank-transform`.

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

When you edit `python/pyproject.toml` (adding a dep, bumping the
version), regenerate the Bazel-side lockfile:

```bash
just batchalign relock
```

**Why isn't this automatic?** `pip.parse` (rules_python) reads
`requirements.lock.txt` at Bazel's *module-resolution phase*, which
happens before any action graph exists. A build action that *produces*
the lockfile runs during the *execution phase*, much later. Bazel
can't do both in one invocation — execution is strictly downstream of
loading. Truly automatic regeneration requires a module extension that
resolves `uv.lock`/`pyproject.toml` at module-resolution time (which is
what `aspect_rules_py`'s `uv` extension does — blocked on a Bazel
version bump). Until then: `just batchalign relock` after pyproject
edits, and `just test` gates drift via the `:requirements_test` target
so CI fails on a missed regen.

### Release a wheel

`python/pyproject.toml [project].version` is the source of truth for
the wheel version. Bump it, regenerate the lockfile, then build a wheel
on each platform:

```bash
just batchalign relock                                       # if deps changed
just batchalign wheel                                        # host-platform wheel
just batchalign wheel-macos-arm64                            # cross-tag (host triple only)
just batchalign wheel-macos-x86_64
just batchalign wheel-linux-x86_64
just batchalign wheel-linux-aarch64
just batchalign wheel-windows-x86_64
just batchalign multiwheel                                   # all of the above
just batchalign publish                                      # twine upload python/target/wheels/*.whl
```

Wheel artifacts land at `python/target/wheels/`. Multi-platform builds
that require cross-compile sysroots succeed locally only for the host
triple; the others are handled by the `.github/workflows/publish-pypi.yml`
CI matrix (5 runners, one per platform, OIDC trusted-publisher to PyPI).

To cut a real release:

1. Bump `python/pyproject.toml [project].version`.
2. `just batchalign relock`.
3. Open a PR; CI runs the lockfile drift test + the Bazel-native py_test.
4. After merge, dispatch `publish-pypi.yml` from the Actions UI with the
   new version as the input `tag` and `publish=true`.

### macOS: `sqlite-unbundled` and homebrew sqlite

`talkbank-transform`'s `Cargo.toml` requests `sqlx-sqlite` via the
`sqlite-unbundled` feature, which tells `libsqlite3-sys` to **link the
system sqlite** instead of compiling its bundled `sqlite3.c`. This
sidesteps a class of macOS SDK header regressions (the 26.x bundle
ships parse-broken `<sys/sysctl.h>` and friends that the cc-rs
invocation can't tolerate outside Bazel's sandboxed `cargo_build_script`).
The Bazel-native build path goes through `rules_rust`'s
`cargo_build_script`, which wires the full hermetic cc toolchain and
*can* compile the bundled sqlite — but the maturin escape path
(`just batchalign wheel` / `sidecar`) uses host cargo and would
choke. Linking system sqlite makes both paths consistent.

The maturin shell scripts auto-set `SQLITE3_LIB_DIR` /
`SQLITE3_INCLUDE_DIR` to homebrew sqlite when present
(`brew install sqlite`). On Linux the distro packages do; on Windows
vcpkg does. If you switch to a setup that genuinely needs the bundled
compile, change the feature back to `sqlite` and the cargo dep
graph regenerates.

### Troubleshooting: macOS SDK / `libsqlite3-sys` build errors

If `just batchalign wheel` / `just batchalign sidecar` on macOS
prints any of these errors, the host setup didn't follow the
[Getting started](#macos) macOS section. The errors fall into two
buckets:

```
sys/proc.h:126:2: error: unknown type name 'u_quad_t'
sys/proc.h:138:17: error: use of undeclared identifier 'MAXCOMLEN'
```

→ libsqlite3-sys tried to compile its bundled `sqlite3.c` against the
SDK. This shouldn't happen with the workspace's `sqlx-sqlite/unbundled`
feature -- if it does, `brew install sqlite` is missing or
`SQLITE3_LIB_DIR` / `SQLITE3_INCLUDE_DIR` weren't picked up. Verify
with `brew --prefix sqlite` and re-run.

```
mach/arm/vm_types.h:104:44: error: expected ';' after top level declarator
sys/sysctl.h:801:22: error: a parameter list without types is only allowed
                            in a function definition  (on `__sized_by`)
sys/event.h:257: error: missing ',' between enumerators (on `__deprecated_enum_msg`)
```

→ The CommandLineTools 26.x clang and SDK are mismatched; *any*
cc-rs-driven crate hits this, not just sqlite. Fix by following the
Xcode-install steps in [Getting started → macOS](#macos). Verifying:

```bash
xcrun --sdk macosx --show-sdk-path
# must be inside Xcode.app, not CommandLineTools
```

Linux + Windows runners ship matched toolchains and are unaffected.

### Hermeticity pins

The maturin/wheel path uses tools outside Bazel's hermetic sandbox
(cargo, host SDK). `bazel/python/hermeticity_guard.sh` asserts
uv/maturin/python/rustc versions match the pins in `python/pyproject.toml
[tool.batchalign.pinned_tools]` before any shell-out, and scrubs
leak-prone env vars (`CFLAGS`, `LDFLAGS`, `DYLD_LIBRARY_PATH`,
`RUSTFLAGS`, `OPENSSL_DIR`, etc.) so a shell with weird state can't
silently produce a divergent wheel. Bumping a tool: update the pin in
`pyproject.toml` AND in `MODULE.bazel` AND in `rust-toolchain.toml` in
the same commit.

#### Hermetic C toolchain (`toolchains_llvm`)

`MODULE.bazel` registers `toolchains_llvm` to pin a specific LLVM/clang
release. Every Bazel-driven C/C++ action uses that clang.

The maturin escape scripts (`bazel/python/{maturin,pyapp}_build.sh`)
pick the C toolchain by host OS:

- **Linux / Windows:** resolve the toolchains_llvm clang/ar/ranlib out
  of the sh_binary runfiles tree and export them as `CC` / `CXX` /
  `AR` / `RANLIB` / `CARGO_TARGET_*_LINKER`. `/usr/bin/cc` is never
  touched.
- **macOS:** defer to Xcode's bundled clang via `xcrun -find clang`.
  Apple ships SDK + clang as a matched pair, and `toolchains_llvm`
  1.7.0 caps its darwin-arm64 prebuilts at LLVM 17.0.6 (LLVM stopped
  publishing arm64-apple-darwin prebuilts on llvm.org), so a hermetic
  clang would be too old for current SDK headers. Xcode is the
  pragmatic answer; see [Getting started → macOS](#macos).

A `BATCHALIGN_FORCE_DARWIN_SDK_WORKAROUND=1` escape hatch in the
shell scripts re-enables the `-D__kernel_ptr_semantics=` CFLAGS
workaround for cross-compile scenarios that target darwin from a
non-darwin host; not needed for normal local builds.

If a future contributor wants fully hermetic macOS builds (CI matrix,
license-clean distribution, defense against the next CLT regression),
the route is `llvm.sysroot()` pointing at an `http_archive` of a
known-good `MacOSX*.sdk` tarball. We don't ship that today because
Apple's EULA prohibits SDK redistribution.

#### Profile selection: Bazel-driven, no env-var rituals

`just batchalign wheel` / `sidecar` / `wheel-<platform>` recipes pass
`-c opt` to `bazel run`, and the `sh_binary` `args` include the
`$(COMPILATION_MODE)` make-variable. The shell scripts translate that to
maturin's `--release` / dev profile. **Do not** prepend
`MATURIN_PROFILE=release` or `PYAPP_PROFILE=release` to the recipe --
Bazel already knows the mode, and re-encoding it in env vars duplicates
the source of truth. `MATURIN_PROFILE` and `PYAPP_PROFILE` remain as
escape hatches when the scripts are run outside Bazel.

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

CLI + LSP ship as platform binaries on GitHub Releases; GUI ships as
signed/notarized `.app`/`.msi`/`.AppImage` bundles via the (TODO)
`publish-chatter.yml` + `publish-desktop.yml` workflows. Manual release
until those land:

```bash
just chatter build                                                    # release builds
cp bazel-bin/crates/chatter/chatter-cli/chatter chatter-$(uname -s)-$(uname -m)
cp bazel-bin/crates/chatter/chatter-lsp/chatter-lsp chatter-lsp-$(uname -s)-$(uname -m)
# upload to a GitHub Release draft

just chatter gui                                                      # GUI bundle
# bundle output at apps/chatter/chatter-gui/src-tauri/target/release/bundle/
# sign + notarize per book/src/operations/code-signing-and-distribution.md
```

VS Code extension:

```bash
just vscode build
just vscode package                                                    # produces .vsix
# Manual upload to marketplace via `vsce publish` (configured token), or
# dispatch the (TODO) publish-vscode.yml workflow.
```

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

## End-to-end smoke from a clean machine (Docker)

Two scenarios that should work on a fresh box without any host-level
batchalign install:

```bash
# 1. Local development path (no maturin):
#    spin up ubuntu:24.04, mount the workspace, run the CLI through
#    Bazel-native py_binary against the rust_shared_library-built .so.
#    Tests that `git clone && just batchalign cli` works on a clean
#    machine.
just docker dev-test

# 2. Released wheel path:
#    build the host wheel via maturin, install it into python:3.12-slim,
#    verify `import batchalign._core` + `batchalign3 --help`. Tests that
#    a downstream PyPI consumer can install + run.
just docker wheel-test

# 3. Both:
just docker e2e
```

The Dockerfiles live at `docker/Dockerfile.dev` and
`docker/Dockerfile.wheel-consumer`. They install the same pinned
toolchain versions (uv, rust, just) used by CI, so a green local
docker e2e is a strong proxy for "CI will also be green".

**Docker Desktop RAM requirement:** `dev-test` compiles the full Rust
workspace inside the container, so the container needs ≥ 8 GiB RAM
(Docker Desktop → Settings → Resources → Memory). Some installs
default to 2 GiB which OOM-kills the Bazel server mid-build. CI
runners are unaffected. `wheel-test` is much lighter (only
`pip install`s a pre-built wheel) and runs in the default config.

**`wheel-test` source of wheels:** by default the recipe builds a host
wheel via `just batchalign wheel` (maturin). If the host can't build
locally (e.g. macOS Command Line Tools SDK issue, or just slow),
drop a CI-built wheel into `python/target/wheels/` first:

```bash
gh run download <run-id> --name batchalign-wheel-linux-aarch64 \
                         -D python/target/wheels/
just docker wheel-test
```

The recipe matches the wheel's platform tag (`manylinux_*_x86_64` →
`docker --platform=linux/amd64`, etc.) and skips the host build when
a wheel is already present.

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

**Bazel is the single entry point.** It orchestrates each ecosystem's
canonical tooling rather than replacing it.

| Surface | Tool Bazel calls | Why |
|---|---|---|
| Rust workspace | `rules_rust` + `crate_universe` | Native Bazel-cached Rust builds. |
| Pyo3 cdylib | `rust_shared_library` + `pyo3-config.txt` | Hand-rolled abi3 path; no `rules_rust_pyo3` toolchain dance (it requires a newer Bazel). |
| Python deps | `rules_uv` `pip_compile` → `rules_python` `pip.parse` | Lockfile drift gated by `:requirements_test`. |
| Python execution | `rules_python` `py_library`/`py_binary`/`py_test` | Bazel-native; `_core.so` carried via runfiles `data`. |
| Python wheel (release) | `maturin` via shell wrapper | No Bazel ruleset packages PyO3 wheels (manylinux, abi3, universal2). Maturin is canonical here. |
| VS Code extension | `npm` + `vsce` via shell wrappers | `vsce` is npm-only. |
| Tauri desktop bundles | `cargo tauri build` via shell wrapper | Tauri's bundling chain (codesign, notarytool, signtool) isn't modelled in Bazel. |
| tree-sitter grammar | `tree-sitter generate` via shell wrapper | Multi-language bindings; only the Rust binding compiles through `cargo_build_script`. |
| mdBook | `mdbook` via shell wrapper | Hermetic `mdbook` binary fetched via multitool. |

Cargo still works at the workspace root (`cargo build`, `cargo nextest
run`, `cargo run -p chatter-cli -- ...`). Bazel is canonical; Cargo is
the escape hatch.

---

## `bazel run` reference

### Chatter

```bash
bazel run //crates/chatter/chatter-cli:chatter           # `chatter` CLI
bazel run //crates/chatter/chatter-lsp:chatter-lsp        # Language Server
bazel run //apps/chatter/chatter-gui/src-tauri:bundle     # cargo tauri build wrapper
```

### Batchalign

```bash
bazel run //python/batchalign                             # `batchalign3` CLI (py_binary)
bazel run //python/batchalign:wheel                       # maturin build → python/target/wheels/
bazel run //python/batchalign:publish                     # twine upload
bazel run //python/batchalign:lint                        # mypy (+ ruff)
bazel test //python/batchalign:pytest                     # pytest via py_test
bazel run //python:requirements                           # regenerate requirements.lock.txt
```

### Spec generators

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
bazel run //bazel/cargo:repin                           # after any Cargo.toml edit
bazel run //bazel/sqlx:prepare                          # after any sqlx::query! edit
```

### Docs

```bash
bazel run //book:serve                                  # preview at http://localhost:3000
bazel run //book:html                                   # static HTML at book/build/html/
bazel run //book:linkcheck                              # mdbook build + linkcheck preprocessor
```

### VS Code extension

```bash
bazel run //apps/vscode-extension:build
bazel run //apps/vscode-extension:package               # produces .vsix
bazel run //apps/vscode-extension:test
```

### Tree-sitter grammar (Rust binding only via Bazel)

```bash
bazel build //grammar:tree_sitter_talkbank
bazel test  //grammar:tree_sitter_talkbank_unit_test
```

---

## Common workflows

### "I edited a Cargo.toml"

```bash
just tooling cargo-repin                                  # = bazel run //bazel/cargo:repin
```

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

```bash
just batchalign relock                                    # regenerate lockfile
# commit pyproject.toml + python/requirements.lock.txt together
```

### "I want a release wheel"

```bash
just batchalign multiwheel                                # builds every supported platform
ls python/target/wheels/
just batchalign publish                                   # twine upload
```

In practice the CI matrix (`publish-pypi.yml`) is the canonical wheel
builder — it fans out to one runner per `(platform, arch)` cell so
every triple gets a native build.

---

## GitHub Actions (namespace-scoped)

`.github/workflows/` is split by surface. Each workflow only runs on
changes affecting its surface (path filters do the gating):

| Workflow | When it runs | What it does |
|---|---|---|
| `bazel-rust.yml` | crates/, grammar/, Cargo.toml, MODULE.bazel | build + unit-test every Rust crate |
| `bazel-python.yml` | python/, batchalign-{core,engine}/, talkbank-{model,parser,transform}/, bazel/python/ | `:requirements_test` drift gate + Bazel-native cdylib + py_library + py_test + CLI smoke |
| `bazel-wheels.yml` | python/, crates/batchalign/, etc. (any path that affects the wheel) | 5-platform wheel matrix on every PR (artifact upload only, no publish); installs wheel into a fresh venv and verifies `import batchalign._core` |
| `bazel-typescript.yml` | apps/vscode-extension/, schemas/ | extension build + .vsix package |
| `bazel-grammar.yml` | grammar/, resources/spec/symbols/ | Rust binding + regen-drift check |
| `bazel-docs.yml` | book/ | mdbook build + linkcheck |
| `bazel-build-all.yml` | cron (06:00 UTC) + manual | `bazel build //...` + `bazel test //...` |
| `publish-pypi.yml` | manual | batchalign wheel matrix (macOS arm/x86, Linux x86/arm, Windows x86) → PyPI (OIDC trusted-publisher) |

All workflows use `bazel-contrib/setup-bazel@0.15.0` for Bazel + cache.

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
