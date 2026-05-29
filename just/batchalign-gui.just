# Batchalign desktop GUI (apps/batchalign/batchalign-gui).
#
# Submodule of `just batchalign`. Discovery:
#   just --list batchalign::gui
#
# Three recipes — all single-shot, no prerequisites:
#   build [profile]   bundle .app/.msi/.AppImage (cargo tauri build, Bazel)
#   dev               cargo tauri dev — hot-reload (Bazel)
#   gen-openapi       regenerate openapi.gen.ts + capabilities.snapshot.json
#
# Both `build` and `dev` declare //python/batchalign:sidecar (a Bazel-
# data dep, so the daemon binary is materialized + staged into
# src-tauri/binaries/sidecar-<triple> by the wrapper script before the
# Tauri bundler / tauri-build sees it. No "build the sidecar first"
# error path — the dep graph IS the prerequisite.

set shell := ["bash", "-c"]
set positional-arguments := true

default:
    @just --list --justfile {{ justfile() }}

# Bundle the desktop GUI (`cargo tauri build`). Default profile: release.
# Resulting .app/.msi/.AppImage ships the daemon embedded.
build profile="release" *args="":
    TAURI_PROFILE={{ if profile == "release" { "release" } else { "debug" } }} \
        bazel run //apps/batchalign/batchalign-gui/src-tauri:bundle -- {{ args }}

# `cargo tauri dev` (hot reload) with the sidecar built + staged by Bazel.
dev *args="":
    bazel run //apps/batchalign/batchalign-gui/src-tauri:dev -- {{ args }}

# Regenerate the OpenAPI-derived TypeScript types + capabilities snapshot
# consumed by the GUI. Pulls the daemon's app.openapi() + GET /capabilities
# and writes:
#   apps/batchalign/batchalign-gui/src/protocol/openapi.gen.ts
#   apps/batchalign/batchalign-gui/src/protocol/capabilities.snapshot.json
# Run after any change to a recipe signature or backend kwargs.
gen-openapi:
    bazel run //apps/batchalign/batchalign-gui:openapi_gen
