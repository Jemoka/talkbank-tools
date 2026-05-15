set shell := ["bash", "-c"]
set positional-arguments := true

build:
    bazel build //crates/chatter/... //apps/chatter/...

test:
    bazel test //crates/chatter/... //apps/chatter/...

cli *args:
    bazel run //crates/chatter/chatter-cli:chatter -- "$@"

lsp *args:
    bazel run //crates/chatter/chatter-lsp:chatter-lsp -- "$@"

gui *args:
    bazel run //apps/chatter/chatter-gui/src-tauri:bundle -- "$@"
