# Releasing

**Status:** Current
**Last updated:** 2026-04-29 10:16 EDT

The extension is published as five platform-specific VSIX files via the
`VS Code Extension Release (Preview)` workflow
(`.github/workflows/vscode-release.yml`). Each VSIX bundles a prebuilt
`chatter-lsp` for its target platform at
`extension/server/chatter-lsp[.exe]`, so end users never need a Rust
toolchain or a PATH binary — the extension finds the bundled server
automatically (see [LSP Binary Discovery](../troubleshooting/lsp.md)). This
workflow creates preview GitHub Releases only; Marketplace publishing is
explicitly out of scope for the first public release.

## Release channel policy

- **Current public channel:** GitHub Releases with platform-specific VSIX files.
- **Not part of the first release:** VS Code Marketplace publish/rollout.
- **Operator rule:** if workflow/docs/release notes say "install from VSIX",
  they must point at the `vscode-vX.Y.Z` GitHub release assets, not a
  Marketplace listing.

## Cutting a release

1. Bump `apps/vscode-extension/package.json` version.
2. Commit and push to `main`.
3. Trigger the workflow:
   ```bash
   gh workflow run vscode-release.yml -f version=X.Y.Z
   ```
4. Wait for all five matrix jobs to succeed:
   - `darwin-arm64`
   - `darwin-x64`
   - `linux-x64`
   - `linux-arm64`
   - `win32-x64`
5. The workflow creates a preview GitHub Release tagged `vscode-vX.Y.Z` with
   the five VSIX files attached.

## Local packaging (single platform)

For a manual build targeting your current host:

```bash
cd <talkbank-tools-root>
cargo build --release -p chatter-lsp
cd vscode
node scripts/prepare-server.mjs --source ../target/release/chatter-lsp
npm run package:darwin-arm64   # or darwin-x64, linux-x64, linux-arm64, win32-x64
```

`prepare-server.mjs` stages the binary into `apps/vscode-extension/server/` with the
correct name (`chatter-lsp` on Unix, `chatter-lsp.exe` on Windows) so
`vsce package` bundles it at `<extension>/server/` inside the VSIX.

## Users installing a VSIX

1. Go to <https://github.com/TalkBank/talkbank-tools/releases>.
2. Open the `vscode-vX.Y.Z` release entry and download the VSIX matching the
   user's platform.
3. Install from the command line:
   ```bash
   code --install-extension talkbank-chat-<platform>.vsix
   ```

No Rust toolchain, no PATH setup, no extra dependencies.

## Version number discipline

`apps/vscode-extension/package.json` is the single source of truth for the extension
version. Do not hand-edit the `extension/server/chatter-lsp` binary
name or the tag format — the CI workflow derives both from `version`.
For a release candidate, append a pre-release identifier (`0.X.Y-rc.1`) per
semver. The workflow still publishes to the `vscode-vX.Y.Z` GitHub prerelease
tag; do not add Marketplace-specific release-channel assumptions.

## Related chapters

- [Installation](../getting-started/installation.md) — user-facing install
- [Troubleshooting: LSP Connection](../troubleshooting/lsp.md) — binary discovery order
- [Testing](testing.md) — the test gates a release must pass
