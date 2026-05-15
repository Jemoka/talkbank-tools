#!/usr/bin/env bash
# Build the batchalign React dashboard SPA via Vite.
# Output: apps/batchalign/batchalign-cli-webdashboard/dist/.
# Node + npm via the rules_nodejs toolchain.
set -euo pipefail
cd "$BUILD_WORKSPACE_DIRECTORY/apps/batchalign/batchalign-cli-webdashboard"
if [ ! -d node_modules ]; then npm install; fi
npm run build
