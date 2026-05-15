#!/usr/bin/env bash
# Build the Chatter desktop app (Tauri v2) end-to-end via `cargo tauri build`.
# Invokes the frontend (npm run build → dist/) and the Rust backend in one
# shot; outputs the platform bundle under
# apps/chatter/chatter-gui/src-tauri/target/release/bundle/.
# cargo + cargo-tauri must be on $PATH (CI installs via setup actions).
set -euo pipefail
cd "$BUILD_WORKSPACE_DIRECTORY/apps/chatter/chatter-gui"
exec cargo tauri build "$@"
