#!/usr/bin/env bash
# Build the Batchalign desktop app (Tauri v2) end-to-end via `cargo tauri build`.
#
# Pre-bundle step: stage the Bazel-built sidecar daemon (built by
# //python/batchalign:sidecar via PyApp) under
# src-tauri/binaries/sidecar-<target-triple>(.exe) so Tauri's
# `bundle.externalBin: ["binaries/sidecar"]` directive can resolve it.
# Tauri v2 appends the host target triple at bundle time; the staged
# filename MUST match exactly.
#
# Profile selection follows Bazel's compilation_mode:
#   BAZEL_COMPILATION_MODE=opt (default) → cargo tauri build              (release bundle)
#   BAZEL_COMPILATION_MODE=dbg            → cargo tauri build --debug      (debug bundle)
# Override directly with TAURI_PROFILE={release|debug} if needed.
#
# Outputs:
#   release → apps/batchalign/batchalign-gui/src-tauri/target/release/bundle/
#   debug   → apps/batchalign/batchalign-gui/src-tauri/target/debug/bundle/
#
# cargo + cargo-tauri must be on $PATH (CI installs via setup actions).
# SIDECAR_PATH is set by the Bazel sh_binary rule via env = {} so we
# don't have to hunt for it in runfiles ourselves.

set -euo pipefail

# Pin RUNFILES_DIR before sourcing runfiles_resolve.sh. Bazel materializes
# the runfiles tree for our sh_binary but doesn't reliably export
# RUNFILES_DIR — and the helper's fallback `${BASH_SOURCE[0]}.runfiles`
# computes against the SOURCED helper, not us. Walk up from $0 to find
# the nearest *.runfiles directory.
if [[ -z "${RUNFILES_DIR:-}" ]]; then
    _self_dir="$(cd "$(dirname "$0")" && pwd -P)"
    _cand="$_self_dir"
    while [[ "$_cand" != "/" ]]; do
        if [[ "${_cand##*/}" == *.runfiles ]]; then RUNFILES_DIR="$_cand"; break; fi
        if [[ -d "${_cand}.runfiles" ]]; then RUNFILES_DIR="${_cand}.runfiles"; break; fi
        _cand="$(dirname "$_cand")"
    done
fi
export RUNFILES_DIR

cd "$BUILD_WORKSPACE_DIRECTORY/apps/batchalign/batchalign-gui"

# Detect target triple. `rustc -vV` emits `host: <triple>` on a line of
# its own; awk pulls just the triple. We need this both for staging the
# sidecar and for the Tauri bundler's --target flag (downstream).
TRIPLE="$(rustc -vV | awk '/^host:/ {print $2}')"
if [[ -z "$TRIPLE" ]]; then
    echo "could not detect rustc host triple" >&2
    exit 2
fi

# Build the sidecar daemon binary via its sh_binary launcher.
#
# //python/batchalign:sidecar is a `sh_binary` (escape-path build —
# pyapp_build.sh needs the host cargo + a writable target dir, which
# can't live in the Bazel sandbox; see bazel/python/pyapp_build.sh).
# Bazel materializes the launcher script into our runfiles tree because
# we list it as `data` on this sh_binary. Executing the launcher runs
# pyapp_build.sh which produces python/target/pyapp/bin/sidecar — the
# stable, deterministic output path the bundler stages from.
#
# SIDECAR_PATH (env, from $(rootpath //python/batchalign:sidecar)) is
# a runfiles-relative path; resolve it to an absolute path inside the
# runfiles tree via the shared helper.
# shellcheck source=../python/runfiles_resolve.sh
source "${BUILD_WORKSPACE_DIRECTORY}/bazel/python/runfiles_resolve.sh"

if [[ -z "${SIDECAR_PATH:-}" ]]; then
    echo "SIDECAR_PATH env var not set (Bazel env= attribute missing?)" >&2
    exit 2
fi
SIDECAR_LAUNCHER="$(runfiles_resolve "$SIDECAR_PATH")"

echo "bundle.sh: building sidecar via $SIDECAR_LAUNCHER"
"$SIDECAR_LAUNCHER"

SIDECAR_BINARY="$BUILD_WORKSPACE_DIRECTORY/python/target/pyapp/bin/sidecar"
if [[ ! -x "$SIDECAR_BINARY" ]]; then
    echo "sidecar launcher ran but did not produce $SIDECAR_BINARY" >&2
    exit 2
fi

mkdir -p src-tauri/binaries
TARGET="src-tauri/binaries/sidecar-${TRIPLE}"
case "$TRIPLE" in
    *windows*) TARGET="${TARGET}.exe" ;;
esac
cp -f "$SIDECAR_BINARY" "$TARGET"
chmod +x "$TARGET"

case "${TAURI_PROFILE:-${BAZEL_COMPILATION_MODE:-opt}}" in
    release|opt) profile_flag=() ;;
    debug|dbg|fastbuild) profile_flag=(--debug) ;;
    *)
        echo "unknown TAURI_PROFILE/BAZEL_COMPILATION_MODE: ${TAURI_PROFILE:-${BAZEL_COMPILATION_MODE}}" >&2
        exit 2
        ;;
esac

exec cargo tauri build "${profile_flag[@]}" "$@"
