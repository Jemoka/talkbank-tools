# Releasing

**Status:** Current
**Last modified:** 2026-06-01 01:05 PDT

The VS Code extension is currently **published manually**. There is no
release workflow in `.github/workflows/`; the only CI for the extension
is `bazel-typescript.yml`, which on every PR builds, tests, and
packages a single-platform `.vsix` and uploads it as a workflow
artifact (`vscode-extension-vsix`).

An automated multi-platform release workflow (with a bundled
`chatter-lsp` per target, GitHub Release upload, and optional
marketplace push) is a planned follow-up. Until then, the process below
is the source of truth.

## CI coverage today

| Workflow | Trigger | What it does |
|---|---|---|
| `bazel-typescript.yml` (`vscode-extension` job) | push/PR on `apps/vscode-extension/**` or `schemas/**` | `bazel run //apps/vscode-extension:build`, `:test`, `:package`; uploads `.vsix` as workflow artifact |

There is **no** `vscode-release.yml`. Any reference to a 5-platform
matrix or `vscode-vX.Y.Z` tagging workflow is aspirational.

## Cutting a release (manual)

1. Bump `apps/vscode-extension/package.json` version (semver).
2. Open a PR with the version bump; merge once green.
3. On a maintainer workstation, from the repo root:
   ```bash
   just vscode build
   just vscode package      # produces apps/vscode-extension/*.vsix
   ```
   Or grab the `vscode-extension-vsix` artifact from the
   `bazel-typescript.yml` run on the merge commit.
4. Publish to the Visual Studio Marketplace using the operator's PAT:
   ```bash
   cd apps/vscode-extension
   npx vsce publish --packagePath talkbank-chat-<version>.vsix
   ```
   The PAT is held by the operator, not in repo secrets. Create at
   <https://dev.azure.com/_usersSettings/tokens> with scope
   "Marketplace > Manage".
5. Optionally attach the `.vsix` to a manually-created GitHub Release
   for users who prefer side-loading.

## Local packaging for testing

Same flow as a release build, just skip the marketplace step:

```bash
just vscode build
just vscode package
code --install-extension apps/vscode-extension/talkbank-chat-<version>.vsix
```

If you want the extension to talk to a locally-built `chatter-lsp`,
make sure that binary is on your `PATH` (e.g. `bazel build
//crates/chatter/chatter-lsp:chatter-lsp` then point `PATH` at
`bazel-bin/...`). Today's `.vsix` does **not** bundle a server binary;
the extension finds `chatter-lsp` via discovery order documented in
[LSP Binary Discovery](../troubleshooting/lsp.md).

## Side-loading a `.vsix` (users)

```bash
code --install-extension talkbank-chat-<version>.vsix
```

Users still need `chatter-lsp` on `PATH`. Bundling a per-platform
`chatter-lsp` into the `.vsix` is part of the planned release-workflow
follow-up.

## Version-number discipline

`apps/vscode-extension/package.json` is the single source of truth.
Pre-release identifiers (`0.X.Y-rc.1`) are allowed per semver. Do not
hand-roll release tags; once an automated workflow exists, it will own
the tag format.

## Planned follow-ups

- A `bazel-vscode-release.yml` (or similar) that:
  - builds `chatter-lsp` per target platform (darwin-arm64,
    darwin-x64, linux-x64, linux-arm64, win32-x64),
  - bundles the matching binary into a per-platform `.vsix`,
  - uploads `.vsix` files to a `vscode-vX.Y.Z` GitHub Release,
  - optionally runs `vsce publish` using a repo-held PAT secret.
- Signed `chatter-lsp` binaries inside each `.vsix` (see
  [Code Signing and Distribution](../../operations/code-signing-and-distribution.md)).

## Related chapters

- [Installation](../getting-started/installation.md) — user-facing install
- [Troubleshooting: LSP Connection](../troubleshooting/lsp.md) — binary discovery order
- [Testing](testing.md) — the test gates a release must pass
- [Release Pipeline](../../operations/release-pipeline.md) — repo-wide artifact map
