#!/usr/bin/env bash
# Compile the VS Code extension TypeScript sources.
# Node + npm are provided by the rules_nodejs toolchain.
set -euo pipefail
cd "$BUILD_WORKSPACE_DIRECTORY/apps/vscode-extension"
if [ ! -d node_modules ]; then npm install; fi
npm run compile
