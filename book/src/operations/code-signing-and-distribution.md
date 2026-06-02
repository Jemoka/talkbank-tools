# Code Signing and Distribution

**Status:** Current
**Last modified:** 2026-06-01 01:05 PDT

This page describes the **current** signing and trust posture of every
artifact produced by this repository. Nothing the repo currently
ships is code-signed, notarized, or marked as a trusted installer on any
platform. Release documentation must reflect that — overclaiming trust
is a documentation bug.

## Current state, per surface

| Surface | Built by | Signed? | Notarized? | Distribution today |
|---|---|---|---|---|
| `batchalign3` Python wheel | `publish-pypi.yml` (manual dispatch) | Wheel itself is not code-signed; PyPI upload uses OIDC trusted publishing for **provenance**, not artifact signing | n/a | PyPI |
| Batchalign desktop bundle (`.app`/`.dmg`/AppImage) | `bazel-tauri-batchalign.yml` bundle matrix | No | No | GitHub Actions workflow artifact only (no public release) |
| VS Code extension `.vsix` | `bazel-typescript.yml` (CI) + manual `vsce publish` | No | n/a | Marketplace push is currently manual from an operator workstation |
| `chatter` / `chatter-lsp` binaries | `bazel-rust.yml` (build only) | No | No | No release workflow exists; users build from source |
| Chatter desktop GUI bundle | `bazel-build-all.yml` (nightly build only) | No | No | No bundle is published anywhere |

PyPI's OIDC trusted-publisher flow proves **the upload came from a
specific GitHub workflow in this repo**; it does not sign the wheel
bytes. Treat OIDC as a deploy-key story, not as artifact signing.

## What docs may claim today

- **Allowed:** "GitHub Release archive", "PyPI wheel",
  "`uv pip install batchalign3`", "VSIX file", "workflow artifact",
  "terminal-first archive", "unsigned development bundle".
- **Not allowed unless we wire the automation in the same patch:**
  "signed", "notarized", "Gatekeeper-trusted",
  "SmartScreen-trusted", "Apple-notarized", "Authenticode-signed",
  "Marketplace-published" (unless an automated marketplace workflow
  exists).

If a release note or install doc claims any of the blocked phrases, it
is a documentation bug — fix the doc or fix the workflow, in the same
patch.

## Gaps and what would unblock them

### macOS (Batchalign + Chatter desktop, chatter CLI)

To ship signed and notarized macOS bundles, the workflow needs:

- Apple Developer ID Application certificate (org-level).
- Secrets in the workflow environment: `APPLE_CERTIFICATE`,
  `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`,
  `APPLE_API_KEY`, `APPLE_API_ISSUER`, `APPLE_TEAM_ID`.
- `tauri-action` (or an equivalent step) wired into the
  `bazel-tauri-batchalign.yml` bundle matrix so signing happens before
  artifact upload.
- Notarization via `notarytool` after signing, before stapling.

`bazel-tauri-batchalign.yml` has a `TODO: signing / notarization`
comment at the bottom that names the same secrets — this is the
follow-up.

### Windows (all surfaces)

Windows is not built at all in CI today:

- No `windows-latest` row in `bazel-tauri-batchalign.yml` (commented
  out — `tools/bazel` bash wrapper incompatible with PowerShell).
- No `windows-x86_64` cell in `bazel-wheels.yml` (commented out —
  multitool's `uv.exe` resolves to `/uv.exe` under bash-on-Windows;
  blocked on a `multitool.lock.json` workaround).

To ship signed Windows artifacts the repo first needs to fix those
portability gaps, then add an EV code-signing certificate and an
Authenticode step on the relevant workflows.

### Linux

For `.deb`/`.rpm`/AppImage distribution, signing would mean GPG-signing
the packages and publishing a public key. Not in scope until a Linux
distribution channel exists; today Linux artifacts are workflow
artifacts only.

### VS Code extension

Marketplace publishing today is manual. The extension is not bundled
with a signed `chatter-lsp` binary; users install from a `.vsix` and
the extension expects `chatter-lsp` on `PATH` or from a known fallback
location. An automated release would also need to settle the
LSP-bundling story (one VSIX per platform with the signed
`chatter-lsp` inside).

## Operator checklist

Before any release goes out:

1. Confirm the workflow output matches the channel claimed in the
   docs.
2. Confirm docs do not overclaim signing / notarization status.
3. If the release is a desktop bundle, mark it explicitly as
   "unsigned development build" in any download instructions.
4. If the channel changes, update this doc, the relevant release
   docs, and the workflow in the same patch.

## See also

- [Release Pipeline](./release-pipeline.md)
- [CI and Release](../contributing/ci-and-release.md)
- [`.github/workflows/bazel-tauri-batchalign.yml`](https://github.com/TalkBank/talkbank-tools/blob/main/.github/workflows/bazel-tauri-batchalign.yml) — see the TODO at the bottom for the signing-secrets shape.
