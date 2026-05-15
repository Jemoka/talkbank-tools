set shell := ["bash", "-c"]
set positional-arguments := true

build:
    bazel build //crates/clan/...

test:
    bazel test //crates/clan/...

core *args:
    bazel run //crates/clan/clan-core:clan_core -- "$@"
