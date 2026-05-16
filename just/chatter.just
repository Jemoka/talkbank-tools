# Chatter — CHAT validation CLI, LSP, and Tauri desktop GUI.
#
# Uniform recipe shape across products (batchalign + chatter):
#   build [profile]   build every Bazel target for the product
#   test  [profile]   run every Bazel test target for the product
#   cli  *args        primary user-facing CLI for the product
#   versions          source-of-truth versions for this product
# Chatter-specific:
#   lsp  *args        run the Language Server
#   gui  [profile]    bundle the desktop app (Tauri)
#
# Profile is `release` (default) or `debug`. Maps to:
#   - Bazel: --config=release | --config=dev
#   - Tauri bundle: (default) | --debug

set shell := ["bash", "-c"]
set positional-arguments := true

# Default: list available recipes.
default:
    @just --list --justfile {{ justfile() }}

# Build every Chatter Bazel target.
build profile="release":
    bazel build --config={{ if profile == "release" { "release" } else { "dev" } }} \
        //crates/chatter/... //apps/chatter/...

# Run every Chatter Bazel test target.
test profile="release":
    bazel test --config={{ if profile == "release" { "release" } else { "dev" } }} \
        //crates/chatter/... //apps/chatter/...

# Chatter CLI (release).
cli *args:
    bazel run --config=release //crates/chatter/chatter-cli:chatter -- "$@"

# Chatter LSP (release).
lsp *args:
    bazel run --config=release //crates/chatter/chatter-lsp:chatter-lsp -- "$@"

# Bundle the desktop app (`cargo tauri build`). Default: release.
gui profile="release" *args="":
    TAURI_PROFILE={{ if profile == "release" { "release" } else { "debug" } }} \
        bazel run //apps/chatter/chatter-gui/src-tauri:bundle -- "$@"

# Print the source-of-truth versions for chatter.
versions:
    @root="{{ justfile_directory() }}"; \
    echo "Chatter CLI + LSP (Cargo.toml [workspace.package].version):"; \
    awk -F'"' '/^\[workspace\.package\]/{f=1;next} f&&/^\[/{exit} f&&/^version *= *"/{print "  " $2;exit}' "$root/Cargo.toml"; \
    echo "Chatter desktop bundle (apps/chatter/chatter-gui/src-tauri/tauri.conf.json):"; \
    awk -F'"' '/"version":/ {print "  " $4; exit}' "$root/apps/chatter/chatter-gui/src-tauri/tauri.conf.json"; \
    echo "Pinned tools (workspace):"; \
    awk -F'"' '/^rust\.toolchain/,/\)/' "$root/MODULE.bazel" | awk -F'"' '/versions/ {print "  rust = " $2; exit}'
