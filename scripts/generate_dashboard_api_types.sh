#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "$ROOT"
cargo run -q -p batchalign-cli --no-default-features --features binary-entry,server -- openapi --output openapi.json

cp "$ROOT/openapi.json" "$ROOT/apps/batchalign/cli-web-statuspage/openapi.json"

cd "$ROOT/frontend"
npx openapi-typescript openapi.json -o src/generated/api.ts

echo "Regenerated dashboard API schema and TypeScript types."
