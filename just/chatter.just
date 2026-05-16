# Chatter — CLI, LSP, desktop GUI (Tauri).
#
# Build profile is `release` by default; pass `debug` for the dbg compile mode.
# The desktop bundle target shells out to `cargo tauri build`, with profile
# threaded through via TAURI_PROFILE / BAZEL_COMPILATION_MODE.
set shell := ["bash", "-c"]
set positional-arguments := true

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
