# Batchalign desktop GUI (apps/batchalign/batchalign-gui).
#
# Submodule of `just batchalign`. Discovery:
#   just --list batchalign::gui
#
# Recipes — all single-shot, no prerequisites:
#   build [profile]   bundle .app/.msi/.AppImage (cargo tauri build, Bazel)
#   dev               cargo tauri dev — hot-reload (Bazel)
#   openapi           generate OpenAPI + capabilities JSON via Bazel
#   test              Playwright end-to-end test against a real sidecar
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

# Generate the protocol artifacts from the daemon's app.openapi() and
# GET /capabilities. These are Bazel outputs, not checked-in source files.
openapi:
    bazel build //apps/batchalign/batchalign-gui:protocol_artifacts

# End-to-end Playwright test. Drives the Vite-served React frontend
# inside Chromium against the real Bazel-built PyApp sidecar daemon.
# Tauri host APIs (invoke / listen / dialog.open) are stubbed; the
# stubs forward to the real daemon for HTTP routes so the test exercises
# the same wire format the production Tauri shell uses.
test:
    bazel build //python/batchalign:sidecar
    cd {{ justfile_directory() }}/apps/batchalign/batchalign-gui && npx playwright test e2e/gui.spec.ts --reporter=list
