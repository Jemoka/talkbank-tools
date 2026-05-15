# ADR-004: Bundled LSP binary in the VSIX

**Status:** Accepted
**Last updated:** 2026-04-29 10:16 EDT

## Context

Per [ADR-001](adr-001-lsp-over-embedded-parser.md), the extension
needs a `chatter-lsp` binary to function. Two distribution shapes:

1. **Zero-install.** Ship one generic VSIX; at activation time,
   either find the binary on `PATH` or run `cargo install
   chatter-lsp`. No binary ships in the VSIX.
2. **Bundled.** Build per-platform VSIXes (five targets: macOS ARM,
    macOS Intel, Linux x86, Linux ARM, Windows x86), each carrying
   its own prebuilt `chatter-lsp`. Distribute those VSIXes from
   GitHub Releases for the first public release.

## Decision

**Bundle per platform.** The extension's release workflow builds five
platform-specific VSIXes and publishes them to a GitHub Release. The
extension's `activate` path
locates the bundled binary via
`vscode.ExtensionContext.extensionUri` before falling back to `PATH`
and finally to the `talkbank.lsp.binaryPath` setting.

The activation fallback order:

1. `talkbank.lsp.binaryPath` setting (user-provided absolute path).
2. Bundled binary at `<extensionPath>/server/chatter-lsp`
   (`.exe` on Windows).
3. `chatter-lsp` on `PATH` (for users who `cargo install`ed).

If all three fail, activation surfaces a
"TalkBank LSP binary not found" message with an "Open Settings"
action pointing at the path setting.

## Consequences

**Positive.**

- First-time install works with zero configuration on any supported
  platform. Non-programmer users (linguists, corpus builders) never
  need a Rust toolchain.
- The extension ships the exact LSP version it was tested against.
  No version-skew between the client protocol and a separately-
  installed server.
- CI pipeline enforces platform coverage: a release that fails to
  build for one target can't ship that platform's VSIX.
- GitHub Releases keeps the first public release path simple: one
  workflow, one release page, no Marketplace credentials or staged
  rollout policy yet.

**Negative.**

- Each VSIX is 10–15 MB (vs. < 1 MB for an extension without a
  binary). Download time perceptible on slow connections.
- The release workflow has a 5-target matrix to maintain. A new
  target (Linux ARM32, FreeBSD, …) is a real change rather than a
  no-op.
- GitHub Releases users must choose the correct asset manually because
  there is no Marketplace auto-targeting for the first release.
- Platform-specific signing (Apple notarization, Windows SmartScreen)
  is the extension's responsibility. Currently signed on macOS;
  Windows relies on publisher reputation rather than code-signing
  certificate.
- The `talkbank.lsp.binaryPath` escape hatch has to keep working —
  developers testing a local LSP build must be able to override the
  bundled binary without uninstalling the VSIX.

## Alternatives considered

**Single generic VSIX + `cargo install` on first run.** Rejected:
users without Rust installed fail at activation; error message
pointing them at `cargo install` is a non-starter for the primary
audience (linguists).

**Download binary at first activation.** Rejected: the download
would need signature verification (otherwise an attacker MITMing
the CDN owns the user's editor), a separate release channel, and
ongoing availability guarantees. GitHub Releases plus bundled VSIXes
keeps the first public release smaller in scope and avoids an extra
distribution service in the activation path.

**Marketplace-first rollout.** Deferred: the first public release is
GitHub Releases VSIX-only so release automation, install docs, and
operator expectations stay aligned while the extension remains
preview-tier.

**Ship source, build with `postinstall`.** Rejected: VS Code
extensions historically do not run post-install scripts; doing so
would require the user to have a Rust toolchain already.

## Source anchors

- Activation binary resolution: `src/activation/lsp.ts`
  (`LSP_BINARY_PATH_SETTING = 'talkbank.lsp.binaryPath'`).
- Release workflow: `.github/workflows/vscode-release.yml`.
- Per-platform build script: `scripts/build-vsix-<target>.sh`.
- Package bundling: `apps/vscode-extension/package.json` `vsce:prepublish` scripts.
- Setting declaration: `apps/vscode-extension/package.json`
  `contributes.configuration` → `talkbank.lsp.binaryPath`.
