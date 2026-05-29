# Contributing to talkbank-tools

**Last updated:** 2026-05-29 10:58 PDT

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

#### macOS (base toolchain)

```bash
# 1. Workspace tools. Bazelisk is what enables tools/bazel
#    (the lockfile-reactivity wrapper) to run.
brew install bazelisk just git

# 2. A working host C toolchain. Xcode Command Line Tools is enough
#    for the base toolchain (batchalign wheel / sidecar / CLI; chatter
#    CLI / LSP via the Bazel-native build path). The wheel build no
#    longer transitively pulls libsqlite3-sys, so the CLT clang/SDK
#    is sufficient on its own.
xcode-select --install                                  # accept dialog if first time

# 3. Optional: faster dev feedback for the VS Code extension.
brew install node                                       # only if you want host-side `npm`
```

That's it. Full Xcode is only required for the Tauri desktop apps;
see [Desktop-GUI host prereqs](#desktop-gui-host-prereqs).

#### Linux (Debian/Ubuntu) (base toolchain)

```bash
sudo apt-get update
sudo apt-get install -y \
    git curl build-essential pkg-config ca-certificates

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

Linux distros ship matched clang/SDK by default; no macOS-style
toolchain caveat applies.

#### Linux (Fedora/RHEL) (base toolchain)

```bash
sudo dnf install -y git gcc gcc-c++ make pkgconf-pkg-config
# Bazel + just same as Debian (apt → dnf for the bazelisk package).
```

#### Windows (base toolchain)

Not tested by maintainers as a daily-driver dev environment, but CI
builds wheels on Windows runners. Bare minimum:

```powershell
# winget or scoop are fine.
scoop install bazelisk just git
# Visual Studio 2022 Build Tools provides MSVC (the `*-sys` crate
# linker target on Windows).
```

WSL2 (Ubuntu) is the easier path if you don't have a hard Windows
requirement -- the Linux instructions above work as-is.

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

#### macOS (Tauri)

Tauri on macOS links against WebKit.framework and other Cocoa
frameworks. The CLT SDK exposes these symbols, but Tauri's bundler
chain (`cargo tauri build` → codesign → notarytool) sometimes hits
SDK-header drift on recent macOS releases; the supported answer is
**full Xcode** for desktop-app contributors:

```bash
# 1. Install Xcode. Recent macOS releases (26.x) ship a CLT bundle
#    whose clang and SDK don't agree with each other — cc-rs-driven
#    builds can fail with `__kernel_ptr_semantics` / `__sized_by` /
#    `fixpt_t` parse errors. Full Xcode bundles a matched clang/SDK.
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
```

We don't vendor an SDK as a workaround because Apple's EULA prohibits
redistribution. The Bazel-native build path goes through
`toolchains_llvm`'s hermetic clang on Linux/Windows; on macOS we
defer to whatever clang `xcrun -find clang` resolves (Xcode or CLT).

#### Linux (Tauri, Debian/Ubuntu)

```bash
sudo apt-get install -y \
    libwebkit2gtk-4.1-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    libssl-dev \
    patchelf

# node is also required for the frontend (Vite + npm install). Bazel's
# rules_nodejs ships a hermetic node for build steps, but `cargo tauri
# dev`'s reload loop currently shells out to host `npm`. Install one:
sudo apt-get install -y nodejs npm
# or use nvm / fnm if you want a specific version
```

#### Linux (Tauri, Fedora/RHEL)

```bash
sudo dnf install -y \
    webkit2gtk4.1-devel \
    libappindicator-gtk3-devel \
    librsvg2-devel \
    openssl-devel \
    patchelf \
    nodejs npm
```

#### Windows (Tauri)

WebView2 ships with Windows 11 and on Windows 10 via the Edge runtime.
No extra installs beyond the base toolchain. Install Node.js separately
if you want `npm` for the dev-server loop.

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

The SQLite validation/roundtrip cache lives in
`crates/core/talkbank-cache` (the `talkbank_cache::CachePool` type,
re-exported as `UnifiedCache`). It uses `sqlx::SqlitePool` via the
`sqlx-sqlite/sqlite-unbundled` feature so it **links** against the
host's libsqlite3 rather than compiling a bundled `sqlite3.c`. The
unbundled path is what lets the macOS CLT 26.x SDK regression (broken
parse of `<sys/sysctl.h>` etc. under cc-rs) not bite this codebase.

**Who pulls in libsqlite3-sys:**

  - chatter-cli, chatter-lsp, chatter-gui (via talkbank-cache) — yes
  - talkbank-transform                                              — no
  - batchalign-core, batchalign-engine, the wheel, the sidecar      — no
  - batchalign-gui                                                  — no

Batchalign has its own `redb`-backed cache
(`crates/batchalign/batchalign-engine/src/cache.rs`) — a pure-Rust
embedded KV store with no system-library dependency. The wheel / CLI
/ sidecar therefore build on a host with no sqlite installed at all.

If you ever need to remove sqlite from chatter too, the
`sqlite-unbundled` feature in `crates/core/talkbank-cache/Cargo.toml`
is the single switch — flipping it back to `sqlite` returns to the
bundled `sqlite3.c` compile (which fails on CLT 26.x but is fine on
Linux/Windows and works on macOS once Xcode is selected).

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
the wheel version. Bump it and build a wheel on each platform — the
lockfile refresh is implicit:

```bash
just batchalign wheel                                        # host-platform wheel (auto-regen if needed)
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
2. Open a PR. The lockfile diff is auto-included on the next Bazel
   invocation; commit it as part of the PR (CI re-runs the
   `:requirements_test` drift gate and fails if you forgot).
3. After merge, dispatch `publish-pypi.yml` from the Actions UI with the
   new version as the input `tag` and `publish=true`.

### Troubleshooting: macOS SDK header errors

The batchalign wheel / sidecar no longer transitively links sqlite,
so the historical `libsqlite3-sys` failure mode is gone. SDK-header
parse errors can still appear in two situations:

```
mach/arm/vm_types.h:104:44: error: expected ';' after top level declarator
sys/sysctl.h:801:22: error: a parameter list without types is only allowed
                            in a function definition  (on `__sized_by`)
sys/event.h:257: error: missing ',' between enumerators (on `__deprecated_enum_msg`)
```

→ CommandLineTools 26.x ships a mismatched clang/SDK pair; any
cc-rs-driven crate that includes those headers (rare in batchalign's
wheel graph, but possible — `ring` is the usual suspect) will
fail. Install full Xcode and select it
(see [Desktop-GUI host prereqs → macOS](#macos-tauri) for the exact
steps). Verify with:

```bash
xcrun --sdk macosx --show-sdk-path
# must be inside Xcode.app, not CommandLineTools
```

The same fix applies when building chatter (`talkbank-cache` links
libsqlite3, and any chatter contributor already needs `brew install
sqlite` from [Chatter-only host prereqs](#chatter-only-host-prereqs)).

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
- **macOS:** defer to whatever clang `xcrun -find clang` resolves
  (Xcode or CommandLineTools — Apple ships the SDK and clang as a
  matched pair inside each). `toolchains_llvm` 1.7.0 caps its
  darwin-arm64 prebuilts at LLVM 17.0.6 (LLVM stopped publishing
  arm64-apple-darwin prebuilts on llvm.org), so a hermetic clang
  would be too old for current SDK headers. For the wheel and CLI
  paths CLT is enough; full Xcode is only required for the Tauri
  desktop apps, see [Desktop-GUI host prereqs](#desktop-gui-host-prereqs).

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
bazel run //bazel/sqlx:prepare                          # after any sqlx::query! edit
```

Lockfile maintenance (`Cargo.lock`, `python/requirements.lock.txt`,
`python/uv.lock`) is handled automatically by `tools/bazel` on every
Bazel invocation — there is no separate command. The
`//bazel/cargo:repin` target still exists as a manual escape hatch but
should not appear in any contributor workflow.

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
