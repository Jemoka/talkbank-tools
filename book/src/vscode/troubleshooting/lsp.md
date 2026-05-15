# LSP Connection

**Status:** Current
**Last updated:** 2026-04-16 13:37 EDT

The TalkBank extension is powered by a Rust language server (`chatter-lsp`) that communicates with VS Code over stdio. If the language server fails to start or crashes, most extension features will not work. This chapter covers how to diagnose and fix LSP connection issues.

## Checking the Output Panel

The first step for any LSP issue is the Output panel:

1. Open the Output panel: **View > Output** or `Cmd+Shift+U` (macOS) / `Ctrl+Shift+U` (Windows/Linux)
2. Click the dropdown in the top-right of the Output panel
3. Select **"TalkBank Language Server"**

This shows the LSP communication logs and any server stderr output. Look for:

- **Startup messages** -- the server logs its version and capabilities on startup
- **Error messages** -- parse failures, missing files, or crashes
- **Connection lost** -- if the server crashed, you will see a disconnection message

## Binary Not Found

**Symptom:** The Output panel shows an error about not finding the `chatter-lsp` binary, or a notification appears with a link to the `talkbank.lsp.binaryPath` setting.

The extension searches for the language-server binary in this order:

1. **`talkbank.lsp.binaryPath` setting** -- if set and the file exists, used as an explicit override
2. **Bundled binary** -- `<extension>/server/chatter-lsp[.exe]`, shipped inside platform-specific VSIXes
3. **System PATH** -- runs `which chatter-lsp` (or `where chatter-lsp` on Windows); used by developers who install via `cargo install`
4. **Dev-tree builds** -- `<extension>/../target/debug/chatter-lsp` or `.../target/release/chatter-lsp`, for contributors running from a cloned workspace
5. If none are found, an actionable error notification appears with a link to the setting

**Fixes:**

- **Users:** install the VSIX that matches your platform (e.g. `talkbank-chat-darwin-arm64.vsix`). Each platform-specific VSIX bundles the correct prebuilt `chatter-lsp`. See [Installation](../getting-started/installation.md).
- **Developers who installed via Cargo:** `cargo install --path crates/chatter-lsp` and confirm `which chatter-lsp` resolves
- **Contributors running from source:** `cargo build --release -p chatter-lsp` so the dev-tree path exists
- **Manual override:** set `talkbank.lsp.binaryPath` in your settings to the absolute path of an existing `chatter-lsp` binary

## Enabling Trace Logging

For detailed server diagnostics, enable trace-level logging:

### Server-side logging (RUST_LOG)

Set the `RUST_LOG` environment variable before launching VS Code:

```bash
RUST_LOG=debug code .
```

Or for more targeted tracing:

```bash
RUST_LOG=chatter_lsp=debug code .
RUST_LOG=chatter_lsp::alignment=trace code .
```

The server uses the `tracing` crate for structured logging. Trace output appears in the Output panel under "TalkBank Language Server".

### LSP message inspection

To see the raw JSON-RPC messages between VS Code and the server, check whether the extension supports the `chatter-lsp.trace.server` setting. Set it to `"verbose"` to log all request/response payloads. Alternatively, use VS Code's built-in LSP inspector if available.

## Server Crash Recovery

The extension automatically restarts the language server if it crashes. You should see a brief interruption in diagnostics and features, followed by automatic recovery.

If the server crashes repeatedly:

1. Check the Output panel for the crash message
2. Note the file you were editing when the crash occurred
3. Try opening a different `.cha` file to determine if the crash is file-specific
4. If the crash is reproducible, file a bug report with:
   - The `.cha` file that triggers the crash (or a minimal reproduction)
   - The Output panel log
   - The `RUST_LOG=debug` output

## Server Hangs (No Response)

**Symptom:** The language server appears to be running but features stop responding (no hover, no diagnostics, no completion).

**Possible causes:**

- **Large file parsing.** Very large CHAT files (thousands of utterances) can take time to parse. Wait a few seconds after opening or editing.
- **Debounce delay.** The server debounces validation by 250ms after each edit. During fast typing, diagnostics are intentionally delayed.
- **Deadlock.** Rare, but possible. Restart VS Code to recover. If reproducible, file a bug report with `RUST_LOG=debug` output.

## Verifying Server Version

To confirm which version of the language server is running, check the Output panel at startup. The server logs its version string. Compare this against the output of:

```bash
chatter --version
```

If the versions do not match, the extension may be using a stale binary. Rebuild or update `talkbank.lsp.binaryPath`.

## Related Chapters

- [Common Issues](common-issues.md) -- general troubleshooting
- [Settings Reference](../configuration/settings.md) -- `talkbank.lsp.binaryPath` setting
- [Architecture](../developer/architecture.md) -- how the LSP server works internally
