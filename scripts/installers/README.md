# Batchalign install helper scripts

**Status:** Current
**Last updated:** 2026-04-29 10:24 EDT

These scripts are convenience wrappers around the canonical public install
command:

```bash
uv tool install batchalign3
```

They help users who already trust the repo download/checkout and want a
double-click path to the same preview PyPI package. They are **not** a separate
signed/native installer channel, and release docs should not present them as
the primary public install story.

## macOS

**File:** `macos/install-batchalign3.command`

1. Download the file.
2. Double-click it in Finder.
3. If macOS Gatekeeper blocks it:
   - Right-click the file > **Open** > **Open** in the dialog.
   - Or: System Settings > Privacy & Security > scroll down > **Open Anyway**.
4. A Terminal window will open showing installation progress.
5. When done, open a **new** Terminal window and run `batchalign3 --help`.

The script installs `uv` (if not present) and then installs `batchalign3`
via `uv tool install`. Re-running the script upgrades an existing installation.

## Windows

**File:** `windows/install-batchalign3.bat`

1. Download the file.
2. Double-click it in Explorer.
3. If Windows SmartScreen blocks it: click **More info** > **Run anyway**.
4. A Command Prompt window will open showing installation progress.
5. When done, open a **new** PowerShell or Command Prompt and run
   `batchalign3 --help`.

The script installs `uv` (if not present) via PowerShell and then installs
`batchalign3` via `uv tool install`. Re-running the script upgrades an
existing installation.

## Release-channel status

| Channel | What it is | Status today |
|---|---|---|
| `uv tool install batchalign3` | Canonical public install path for the `0.1.x` preview line | Allowed |
| PyPI wheel/sdist | Package payload used by the `uv` install flow | Allowed |
| Optional GitHub Release wheel/sdist attachments | Alternate download for the same package when published by the release workflow | Allowed, but secondary to the `uv` path |
| This directory's `.command` / `.bat` files | Convenience wrappers that install `uv` if needed and then run `uv tool install batchalign3` | Helper-only; not a separate signed installer tier |
| Native installers (`.pkg`, `.dmg`, `.exe`, `.msi`) | OS-level installer surfaces | Blocked until signing/notarization automation exists |

Use **public preview** wording consistently. `batchalign3` is intentionally
pre-1.0, and these scripts should be described as wrappers around the preview
CLI install flow, not as a promoted native-installer story.

## Testing

Both scripts support `BATCHALIGN_PACKAGE` (override package spec) and `CI=true`
(skip interactive prompts) environment variables for automated testing.

```bash
# Test the macOS installer (builds wheel, isolated sandbox, cleanup)
bash scripts/installers/test.sh

# Reuse an existing wheel in dist/
bash scripts/installers/test.sh --no-build

# Test the full GitHub Release flow (creates draft release, downloads, installs)
bash scripts/installers/test-github-release.sh
bash scripts/installers/test-github-release.sh --no-build
```

Both test scripts use `UV_TOOL_DIR`/`UV_TOOL_BIN_DIR` to install into an
isolated temp directory — they do not affect the developer's real tool
installations. The GitHub Release test creates a draft pre-release, verifies
the download + install path, then deletes the draft on cleanup.
