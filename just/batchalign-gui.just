# Batchalign desktop GUI (apps/batchalign/batchalign-gui).
#
# Submodule of `just batchalign`. Discovery:
#   just --list batchalign::gui
#
# Three recipes:
#   build [profile]   bundle .app/.msi/.AppImage via cargo tauri build (Bazel-orchestrated)
#   dev               cargo tauri dev — hot-reload, no Bazel
#   gen-openapi       regenerate openapi.gen.ts + capabilities.snapshot.json
#
# Both `build` and `dev` need the sidecar daemon staged under
# src-tauri/binaries/sidecar-<target-triple>. `build` stages it via
# bazel/batchalign-tauri/bundle.sh; `dev` copies it on-the-fly from the
# Bazel sidecar output (or asks the user to `just batchalign sidecar`
# first if it doesn't exist).

set shell := ["bash", "-c"]
set positional-arguments := true

default:
    @just --list --justfile {{ justfile() }}

# Bundle the desktop GUI (`cargo tauri build`). Default profile: release.
#
# The sidecar daemon (//python/batchalign:sidecar) is staged under
# apps/batchalign/batchalign-gui/src-tauri/binaries/sidecar-<triple> by
# bazel/batchalign-tauri/bundle.sh before the Tauri bundler runs, so the
# resulting .app/.msi/.AppImage ships the daemon embedded.
build profile="release" *args="":
    TAURI_PROFILE={{ if profile == "release" { "release" } else { "debug" } }} \
        bazel run //apps/batchalign/batchalign-gui/src-tauri:bundle -- "$@"

# `cargo tauri dev` with hot reload. Bypasses Bazel — runs straight against
# the live working tree. Stages the sidecar binary into src-tauri/binaries/
# first (mirrors what bundle.sh does for the release path) so the
# externalBin contract is satisfied at compile time. If the sidecar isn't
# yet built, asks the user to run `just batchalign sidecar`.
dev *args="":
    #!/usr/bin/env bash
    set -euo pipefail
    root="{{ justfile_directory() }}"
    triple="$(rustc -vV | awk '/^host:/ {print $2}')"
    sidecar_src="$root/python/target/pyapp/bin/sidecar"
    if [[ ! -x "$sidecar_src" ]]; then
        echo "sidecar binary not found at $sidecar_src" >&2
        echo "build it first: just batchalign sidecar" >&2
        exit 2
    fi
    dest_dir="$root/apps/batchalign/batchalign-gui/src-tauri/binaries"
    mkdir -p "$dest_dir"
    dest="$dest_dir/sidecar-$triple"
    case "$triple" in
        *windows*) dest="$dest.exe" ;;
    esac
    cp -f "$sidecar_src" "$dest"
    chmod +x "$dest"
    cd "$root/apps/batchalign/batchalign-gui"
    [[ -d node_modules ]] || npm install
    cargo tauri dev -- "$@"

# Regenerate the OpenAPI-derived TypeScript types + capabilities snapshot
# consumed by the GUI. Pulls the daemon's app.openapi() + GET /capabilities
# and writes:
#   apps/batchalign/batchalign-gui/src/protocol/openapi.gen.ts
#   apps/batchalign/batchalign-gui/src/protocol/capabilities.snapshot.json
# Run after any change to a recipe signature or backend kwargs.
gen-openapi:
    bazel run //apps/batchalign/batchalign-gui:openapi_gen
