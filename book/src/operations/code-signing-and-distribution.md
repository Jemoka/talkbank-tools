# Code Signing and Distribution

**Status:** Current
**Last updated:** 2026-04-29 10:16 EDT

This document defines the first-release trust and distribution policy for the
public artifacts shipped from this repository. It is intentionally narrower
than a future full signing playbook: the goal here is to say which channels are
allowed now, which claims release docs may make, and which channels stay
blocked until signing/notarization automation exists.

## Current release channels

| Surface | Current distribution channel | First-release policy |
|---|---|---|
| TalkBank core CLI / `chatter-lsp` | GitHub Release tar/zip archives from `.github/workflows/release.yml` | Allowed as terminal-first archives. Do not describe them as native installers. |
| `batchalign3` | PyPI wheels + sdist, plus optional GitHub Release wheel/sdist attachments from `.github/workflows/batchalign-release.yml` | Allowed. Release smoke must verify the packaged `batchalign3 serve` `/health` path. |
| VS Code extension | GitHub Releases VSIX files from `.github/workflows/vscode-release.yml` | Allowed. First public release is VSIX-only, not Marketplace-first. |
| Desktop / native installers | `.app`, `.pkg`, `.dmg`, `.exe`, `.msi`, or similar double-click install surfaces | Blocked for public release until signing policy and automation are implemented for that surface. |

## What release docs may claim today

- **Allowed language:** GitHub Release archive, VSIX download, PyPI wheel, `uv`
  install, terminal-first archive.
- **Blocked language unless the workflow actually does it:** signed,
  notarized, SmartScreen-trusted, native installer, App Store / Marketplace
  rollout.
- **VS Code:** tell users to install the matching `.vsix` from the
  `vscode-vX.Y.Z` GitHub release. Do not point first-release docs at the VS
  Code Marketplace.

## Signing/notarization gate

The following channels are blocked until explicit signing automation exists and
this document is updated in the same patch:

1. **macOS GUI/direct-download app bundles** (`.app`, `.pkg`, `.dmg`, direct
   double-click CLI wrappers marketed as native installers).
2. **Windows native installers** (`.exe`, `.msi`) that are presented as
   end-user install surfaces rather than terminal commands.
3. **Any release note or install doc** that claims a GitHub Release artifact is
   signed/notarized when the workflow does not perform that step.

## Operator checklist

Before a release goes out:

1. Confirm the workflow output matches the channel described in the docs.
2. Confirm docs do not overclaim signing/notarization status.
3. If the artifact is a direct-download native installer, stop — that surface
   is not yet in the allowed first-release set.
4. If the channel changes, update this doc, the relevant release docs, and the
   workflow in the same patch.
