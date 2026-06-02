# Installation

**Status:** Current
**Last updated:** 2026-06-01 00:52 PDT

This chapter walks you through installing VS Code and the TalkBank CHAT
extension. The language server (`chatter-lsp`) ships inside each
platform-specific VSIX, so there is no separate binary to install. By the end,
you will have a working environment where opening any `.cha` file gives you
syntax highlighting, real-time validation, and all the features described in
this book.

---

## Prerequisites

You need:

1. **Visual Studio Code Insiders** version 1.110 or later
2. **A platform-specific `.vsix` package** — each release publishes five
   VSIXes (darwin-arm64, darwin-x64, linux-x64, linux-arm64, win32-x64)
   with a prebuilt `chatter-lsp` bundled inside

The extension is a `.vsix` package installed into VS Code. Each
platform-specific VSIX bundles the matching `chatter-lsp` binary at
`<extension>/server/chatter-lsp[.exe]`, so end users do not need a Rust
toolchain or any PATH setup.

---

## Step 1: Install VS Code Insiders

### macOS

Install via Homebrew:

```bash
brew install --cask visual-studio-code@insiders
```

This gives you the `code-insiders` command in your terminal.

### Windows

Download from <https://code.visualstudio.com/insiders/>. Run the installer and
ensure "Add to PATH" is checked so the `code-insiders` command is available from
the terminal.

### Linux

Download the `.deb` or `.rpm` package from
<https://code.visualstudio.com/insiders/>, or use snap:

```bash
sudo snap install code-insiders --classic
```

---

## Step 2: Install the Extension

### Option A: Platform-specific VSIX (recommended)

1. Open <https://github.com/TalkBank/talkbank-tools/releases>
2. Choose the `vscode-vX.Y.Z` release entry and download the `.vsix` that
   matches your host:
   - `talkbank-chat-darwin-arm64.vsix` (Apple Silicon Macs)
   - `talkbank-chat-darwin-x64.vsix` (Intel Macs)
   - `talkbank-chat-linux-x64.vsix`
   - `talkbank-chat-linux-arm64.vsix`
   - `talkbank-chat-win32-x64.vsix`
3. Install it:

   ```bash
   code-insiders --install-extension talkbank-chat-<platform>.vsix
   ```

   Or from inside VS Code: open the Command Palette (`Cmd+Shift+P`), type
   **"Extensions: Install from VSIX..."**, and select the `.vsix` file.

Each platform VSIX bundles a matching `chatter-lsp` binary. No Rust
toolchain or PATH setup is required.

### Option B: Development mode (building from source)

Contributors running from a cloned workspace can build `chatter-lsp` and
launch the extension directly:

```bash
# From the talkbank-tools/ repository root:
cargo build --release -p chatter-lsp

cd apps/vscode-extension
npm install
npm run compile     # or: just vscode build (from repo root)

# Launch VS Code with the extension loaded in development mode:
code-insiders --extensionDevelopmentPath=.
```

In this mode the extension picks up the dev-tree build at
`../target/release/chatter-lsp` (or `target/debug/` after `cargo build -p
chatter-lsp`). Producing a full set of platform VSIXes is covered in
the [Releasing chapter](../developer/releasing.md).

---

## Step 3: Verify the Installation

1. Open VS Code Insiders
2. Open any `.cha` file (if you have the repository, try one from
   `resources/corpus/reference/`)
3. You should see:
   - **Syntax highlighting** -- headers, speaker codes, dependent tiers, and
     annotations are colored by category
   - **Validation diagnostics** -- any errors or warnings appear as squiggly
     underlines and in the Problems panel (`Cmd+Shift+M`)
   - **"CHAT Validation" tree view** in the Explorer sidebar

> **(SCREENSHOT: VS Code Insiders with a CHAT file open)**
> *Capture this: a `.cha` file from `resources/corpus/reference/` open in the editor,
> showing syntax highlighting, the Problems panel with diagnostics, and the
> CHAT Validation tree view in the Explorer sidebar.*

If you see plain, uncolored text, the extension did not activate. Check:

- The file has a `.cha` extension
- The language mode in the bottom status bar says "CHAT" (not "Plain Text")
- The Output panel (`Cmd+Shift+U`) shows a "TalkBank Language Server"
  channel with server startup messages

---

## How the LSP Binary Is Found

When the extension starts, it searches for the `chatter-lsp` binary in this order:

1. **`talkbank.lsp.binaryPath` setting** -- if set and the file exists, that
   path is used directly (explicit override)
2. **Bundled binary** -- `<extension>/server/chatter-lsp[.exe]`, shipped with
   each platform-specific VSIX
3. **System PATH** -- `which chatter-lsp`, for developers who install via
   `cargo install`
4. **Dev-tree builds** -- `<extension>/../target/debug/chatter-lsp` or
   `.../target/release/chatter-lsp`, for contributors running from a cloned
   workspace
5. If none of these find a binary, an error notification appears with a link
   to the `talkbank.lsp.binaryPath` setting

The binary is spawned with no arguments. It speaks the Language Server Protocol
over stdio as soon as it starts.

See [Settings Reference](../configuration/settings.md) for the
`talkbank.lsp.binaryPath` setting, and [LSP Connection](../troubleshooting/lsp.md)
for troubleshooting a missing binary.

---

## Next Steps

With the extension installed, open a CHAT file and follow along with
[Your First CHAT File](first-file.md) to explore the core features.
