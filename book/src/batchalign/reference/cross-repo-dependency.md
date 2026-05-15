# Cross-Repository Compatibility Contract

**Status:** Current
**Last updated:** 2026-04-02 07:32 EDT

## Overview

`batchalign3` depends on `talkbank-tools` Rust crates via local path dependencies. This document defines the compatibility contract between the two repositories.

## Current Dependency

```toml
# crates/batchalign/Cargo.toml
talkbank-model = { path = "../../../talkbank-tools/crates/talkbank-model" }
talkbank-parser = { path = "../../../talkbank-tools/crates/talkbank-parser" }
```

This is a **development-only arrangement**. For public release, dependencies must be versioned (see Release Boundary below).

## Consumed Crates

| Crate | Used By | Purpose |
|-------|---------|---------|
| `talkbank-model` | `batchalign`, `batchalign-core` (PyO3) | CHAT data model, validation, alignment types |
| `talkbank-parser` | `batchalign`, `batchalign-core` (PyO3) | CHAT parsing (tree-sitter) |
| `talkbank-transform` | `batchalign` | Parse+validate pipeline, CHAT serialization |

## Compatibility Rules

1. **Breaking changes in talkbank-tools crate APIs require coordinated updates in batchalign3.** Both repos must be updated and tested together.

2. **Synchronized release notes are required** when a cross-repo boundary change happens. Both repos' changelogs must reference the coordinated change.

3. **CI in batchalign3 clones talkbank-tools at HEAD.** This means batchalign3 CI implicitly tests against the latest talkbank-tools. A breaking change in talkbank-tools will surface as a batchalign3 CI failure.

4. **After pulling talkbank-tools changes**, rebuild batchalign3's Python extension: `bazel build //...-python`

## Release Boundary (Target)

Before batchalign3 1.0 public release, the path dependencies must be replaced with one of:

| Option | Pros | Cons |
|--------|------|------|
| **Publish to crates.io** (recommended) | Clean versioning, standard Rust ecosystem | Requires talkbank-tools release first |
| **Git SHA pins** | No crates.io publish needed | Harder to audit, no semver enforcement |
| **Vendor** | Fully self-contained | Drift risk, duplication |

The recommended path: release talkbank-tools crates to crates.io first, then depend on versioned crate releases.

## Release Manifest

Each batchalign3 release must record:
- batchalign3 version and git SHA
- talkbank-tools version (or git SHA) used for the build
- Build date and CI run URL
- License metadata for both repos

This manifest should be generated automatically by the release workflow and included in the GitHub Release notes.
