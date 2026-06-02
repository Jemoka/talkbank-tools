# Installation

**Status:** Current
**Last updated:** 2026-05-02 10:00 EDT

`batchalign3` is a **public preview** product line on the `0.1.x` release
line. The canonical public install path is:

```bash
uv tool install batchalign3
```

That command installs the preview wheel from PyPI. Repo-hosted `.command` and
`.bat` scripts wrap the same flow for users who want a download-and-double-click
helper, but they are not a separate signed/native installer channel.

Batchalign runs on **Windows, macOS, and Linux**. Pre-built wheels are
available for all three platforms (macOS ARM + Intel, Linux x86 + ARM, Windows
x86).

## System requirements

| Requirement | Details |
|------------|---------|
| Python | 3.12 (installed automatically by `uv`) |
| Disk space | ~2 GB for ML models (downloaded on first use) |
| RAM | 8 GB minimum, 16 GB recommended |
| FFmpeg | Only needed for MP4 media files |
| Platforms | macOS ARM + Intel, Windows x86, Linux x86 + ARM |

## Install with `uv`

If you don't have `uv` yet, install it first — then install Batchalign:

```bash
uv tool install batchalign3
```

At this stage there is intentionally **not** a separate
`uv tool install batchalign3-server` package. The default distribution remains
one install so BA2-style local/direct use stays simple while the unreleased
server/control-plane architecture is still evolving.

## Optional helper scripts

If you want a repo-hosted wrapper around the same `uv` install flow:

- **macOS:** [Download install-batchalign3.command](https://github.com/TalkBank/talkbank-tools/raw/main/scripts/installers/macos/install-batchalign3.command)
- **Windows:** [Download install-batchalign3.bat](https://github.com/TalkBank/talkbank-tools/raw/main/scripts/installers/windows/install-batchalign3.bat)

Those scripts install `uv` if needed and then run `uv tool install
batchalign3`. They are convenience helpers only. For release-policy details and
Gatekeeper/SmartScreen notes, see the [installers README](https://github.com/TalkBank/talkbank-tools/blob/main/scripts/installers/README.md).

### macOS

1. Open **Terminal** (search "Terminal" in Spotlight, or find it in
   Applications > Utilities).

2. Install `uv`:
   ```bash
   curl -LsSf https://astral.sh/uv/install.sh | sh
   ```

3. **Close and reopen Terminal** so the new command is available.

4. Install Batchalign:
   ```bash
   uv tool install batchalign3
   ```

5. Verify:
   ```bash
   batchalign3 --help
   ```

### Windows

1. Open **PowerShell** (search "PowerShell" in the Start menu).

2. Install `uv`:
   ```powershell
   irm https://astral.sh/uv/install.ps1 | iex
   ```

3. **Close and reopen PowerShell** so the new command is available.

4. Install Batchalign:
   ```powershell
   uv tool install batchalign3
   ```

5. Verify:
   ```powershell
   batchalign3 --help
   ```

### Linux

```bash
curl -LsSf https://astral.sh/uv/install.sh | sh
source ~/.bashrc   # or restart your terminal
uv tool install batchalign3
batchalign3 --help
```

## First run

After installing, **restart your terminal** so the `batchalign3` command is
on your PATH. The first time you run a processing command (e.g. `morphotag`),
ML models will be downloaded automatically. This is a one-time cost of ~2 GB
and may take a few minutes depending on your connection. Subsequent runs use
cached models.

**Evaluating the experimental GUI shell?** See
[Batchalign Desktop (Experimental)](desktop-app.md). The supported first-time
user path today is still the `batchalign3` CLI path above.

## Updating

Upgrade to the latest version:

```bash
uv tool upgrade batchalign3
```

If you installed via one of the helper scripts, re-running the same script
upgrades the same `uv`-managed installation.

The CLI prints a notice when a newer version is available on PyPI. You can
suppress this by setting `BATCHALIGN_NO_UPDATE_CHECK=1`.

## Offline / alternative install

All install paths use the same package payload. If you need to install without
internet access (e.g., air-gapped machines), you can install from a local wheel
file:

```bash
uv tool install ./batchalign3-0.3.0-cp312-cp312-macosx_11_0_arm64.whl
```

Wheel files for all 5 supported platforms are built by the release CI. If a
preview release publishes wheel attachments on GitHub Releases, you can point
`uv` at that downloaded wheel file as well; the canonical public story remains
the `uv tool install batchalign3` path above.

## Built-in engines

All built-in engines, including Cantonese providers, are part of the base
package:

```bash
uv tool install batchalign3
```

## Worker Python resolution

The CLI finds a Python 3.12 runtime automatically — via
`BATCHALIGN_PYTHON`, the active virtualenv, a sibling/project `.venv`, or
`python3.12` on PATH. Override explicitly:

```bash
# macOS / Linux
export BATCHALIGN_PYTHON=/path/to/venv/bin/python

# Windows (PowerShell)
$env:BATCHALIGN_PYTHON = "C:\path\to\venv\Scripts\python.exe"
```

Under `uv tool install`, the visible `batchalign3` command is still a thin
Python launcher, but it immediately `exec`s the packaged Rust CLI binary. The
wrapper also preserves the chosen Python runtime for worker subprocesses, so
`batchalign3 serve ...` and background/daemon flows still run through the same
Rust CLI/server codepath as direct invocation of the packaged binary.

## Verify the installation

Confirm the CLI is available:

```bash
batchalign3 --help
```

Confirm the chosen Python runtime can import the worker package:

```bash
$BATCHALIGN_PYTHON -c "import batchalign.worker"
```

If you are relying on `VIRTUAL_ENV` or `python3` instead of
`BATCHALIGN_PYTHON`, run the same import check with that interpreter.

## Rev.AI setup

If you plan to use the default Rev.AI-backed transcription path, initialize
`~/.batchalign.ini`:

```bash
batchalign3 setup
```

See [Rev.AI Integration](rev-ai.md) for details.

## Development install

For contributors working from a source checkout:

```bash
git clone https://github.com/TalkBank/talkbank-tools.git
cd talkbank-tools
just batchalign develop    # build wheel + sync uv env + install
bazel build //...                         # full workspace cargo build (release)
```

Batchalign source now lives inside this repository; there is no separate active
`batchalign3` source checkout.

`just batchalign develop` rebuilds the wheel via the maturin backend
declared in `pyproject.toml`, runs `uv sync --group dev --no-install-project`,
and installs the freshly built wheel into the dev environment. The same
target covers Cantonese providers — they are part of the base package.

`bazel build //...` runs `cargo build --workspace --release` plus the spec-tools
build. It does not rebuild the embedded dashboard. If you also need the
React dashboard rebuilt, run `just batchalign dashboard` (which
requires Node.js + npm in addition to Rust and uv).

In a source checkout, `uv run batchalign3` is still the normal way to
invoke the console script — the maturin backend's `profile = "dev"`
setting means each `uv run …` triggers an incremental rebuild of the
PyO3 extension on demand.

For the fastest contributor loop:

```bash
uv run batchalign3 --help        # incremental PyO3 rebuild via maturin/uv
cargo build -p batchalign-cli         # native batchalign3 binary (debug)
./target/debug/batchalign3 --help
```

Reserve `uv run` for Python tools such as `pytest`, `mypy`, and `maturin`
when you are not invoking the CLI itself.

Common rebuilds from a dev checkout:

```bash
cargo build -p batchalign-cli                              # CLI / server changes (debug)
cargo build --workspace --release                      # full release build (slow)
just batchalign develop                          # rebuild + reinstall the wheel
bazel build //...                                              # full workspace + spec tools (release)
./target/debug/batchalign3 --help
cargo run -p batchalign-cli -- --help
cargo nextest run --workspace
cargo nextest run --manifest-path crates/batchalign-pyo3/Cargo.toml
uv run pytest
uv run mypy
```

For the fuller contributor workflow and rebuild matrix, see
[Building & Development](../developer/building.md).
