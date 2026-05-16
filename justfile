# Workspace-wide runner. Per-product recipes live under `just/<product>/...`;
# this hub exposes the cross-cutting build/test commands.
#
# Discovery: `just --list` for the hub, `just --list <scope>` for each scope.
# Two top-level products:
#   - batchalign  (Rust engine + Python wheel)   → `just --list batchalign`
#   - chatter     (Rust CLI + LSP + Tauri GUI)   → `just --list chatter`
# Plus per-surface scopes: clan, spec, vscode, docs, tooling.

set shell := ["bash", "-c"]
set positional-arguments := true

mod chatter    "just/chatter.just"
mod batchalign "just/batchalign.just"
mod clan       "just/clan.just"
mod spec       "just/spec.just"
mod vscode     "just/vscode.just"
mod docs       "just/docs.just"
mod tooling    "just/tooling.just"

default:
    @just --list

# Build every Bazel target in the workspace.
# Profile: `release` (default; opt build, stripped) or `debug` (dbg build, fast).
build profile="release":
    bazel build --config={{ if profile == "release" { "release" } else { "dev" } }} //...

# Run every Bazel test target in the workspace.
test profile="release":
    bazel test --config={{ if profile == "release" { "release" } else { "dev" } }} //...

# Print all product versions (source-of-truth view).
versions:
    @just batchalign versions
    @echo ""
    @echo "Chatter (root Cargo.toml [workspace.package].version, applies to chatter-cli + chatter-lsp + clan-core + talkbank-*):"
    @awk -F'"' '/^\[workspace.package\]/,/^\[/ {if (/^version *= *"/) {print "  " $2; exit}}' Cargo.toml
    @echo ""
    @echo "Chatter GUI bundle (apps/chatter/chatter-gui/src-tauri/tauri.conf.json):"
    @awk -F'"' '/"version":/ {print "  " $4; exit}' apps/chatter/chatter-gui/src-tauri/tauri.conf.json
