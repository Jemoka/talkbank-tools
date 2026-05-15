#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FRONTEND_DIR="$ROOT/frontend"
E2E_DIR="$FRONTEND_DIR/e2e"

if ! command -v npm >/dev/null 2>&1; then
  echo "error: npm not found. Install Node.js + npm first." >&2
  exit 127
fi

"$ROOT/scripts/generate_dashboard_api_types.sh"

cd "$FRONTEND_DIR"
npm ci
npm run build

cd "$E2E_DIR"
npm ci

if [[ "${BATCHALIGN_SKIP_BROWSER_INSTALL:-0}" != "1" ]]; then
  if [[ "${BATCHALIGN_PLAYWRIGHT_WITH_DEPS:-0}" == "1" ]]; then
    npx playwright install --with-deps chromium
  else
    npm run install:browsers
  fi
fi

if [[ "${BATCHALIGN_REAL_SERVER_E2E:-0}" == "1" ]]; then
  BATCHALIGN_BIN_PATH="${BATCHALIGN_BIN:-$ROOT/target/release/batchalign3}"
  if [[ ! -x "$BATCHALIGN_BIN_PATH" ]]; then
    echo "batchalign3 release binary not found at $BATCHALIGN_BIN_PATH; building..."
    cargo build --manifest-path "$ROOT/Cargo.toml" -p batchalign-cli --release
  fi
  export BATCHALIGN_BIN="$BATCHALIGN_BIN_PATH"
  export BATCHALIGN_DASHBOARD_DIR="${BATCHALIGN_DASHBOARD_DIR:-$FRONTEND_DIR/dist}"
fi

npm test
