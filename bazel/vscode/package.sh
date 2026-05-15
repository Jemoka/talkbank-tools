#!/usr/bin/env bash
# Build a .vsix bundle ready for the VS Code Marketplace.
# Requires the chatter-lsp binary first:
#   bazel build //crates/chatter/chatter-lsp:chatter-lsp
# vsce is an npm package; `npx vsce` resolves it from node_modules.
set -euo pipefail
cd "$BUILD_WORKSPACE_DIRECTORY/apps/vscode-extension"
if [ ! -d node_modules ]; then npm install; fi
npx vsce package "$@"
