#!/usr/bin/env bash
#
# Bazel `--workspace_status_command` script.
#
# Emits key/value pairs into volatile-status.txt (no prefix) and
# stable-status.txt (`STABLE_` prefix). Keys are referenced from BUILD
# files via `rustc_env = {"FOO": "{KEY}"}` plus `stamp = 1`.
#
# `STABLE_GIT_HASH` — the short SHA, suffixed `-dirty` if there are
# uncommitted changes. Stable means a SHA change retriggers rebuilds of
# targets that consume it (which is what we want for version stamping:
# the embedded SHA must match the binary that was actually built).
#
# `BUILD_HASH` — same value, volatile. Kept for back-compat with any
# rule that still references `{BUILD_HASH}`.

sha="$(git rev-parse --short HEAD)$(git diff --quiet || echo '-dirty')"
echo "STABLE_GIT_HASH ${sha}"
echo "BUILD_HASH ${sha}"
