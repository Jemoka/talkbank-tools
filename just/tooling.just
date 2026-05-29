set shell := ["bash", "-c"]
set positional-arguments := true

# Lockfile regeneration (Cargo.lock, python/requirements.lock.txt,
# python/uv.lock) is automatic — `tools/bazel` detects staleness on
# every Bazel invocation. No just recipe is needed; if you ever need
# to force a refresh out-of-band the Bazel targets are
# //bazel/cargo:repin and //python:requirements.

xtask *args:
    bazel run //crates/xtask:xtask -- "$@"

sqlx-prepare:
    bazel run //bazel/sqlx:prepare
