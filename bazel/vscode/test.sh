#!/usr/bin/env bash
# Run the VS Code extension test suite.
set -euo pipefail
cd "$BUILD_WORKSPACE_DIRECTORY/apps/vscode-extension"
if [ ! -d node_modules ]; then npm install; fi
npm test "$@"
