set shell := ["bash", "-c"]
set positional-arguments := true

xtask *args:
    bazel run //crates/xtask:xtask -- "$@"

cargo-repin:
    bazel run //bazel/cargo:repin

sqlx-prepare:
    bazel run //bazel/sqlx:prepare
