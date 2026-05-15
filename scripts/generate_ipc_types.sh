#!/usr/bin/env bash
# Generate Python Pydantic models from Rust IPC types via JSON Schema.
#
# Pipeline: Rust (schemars) → JSON Schema → datamodel-codegen → Pydantic
#
# Usage: bash scripts/generate_ipc_types.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

CODEGEN="uvx --from datamodel-code-generator datamodel-codegen"
COMMON_OPTS=(
    --input-file-type jsonschema
    --output-model-type pydantic_v2.BaseModel
    --target-python-version 3.12
    --use-annotated
    --field-constraints
    --collapse-root-models
    --use-title-as-name
)

echo "Step 1: Generating JSON Schema from Rust types..."
cargo run -q -p batchalign-cli -- ipc-schema --output schemas/ipc/

echo "Step 2: Generating Python Pydantic models from JSON Schema..."
mkdir -p batchalign-cli/generated/worker_v2 python/batchalign/generated/batch_items

# Worker V2 types (directory → directory, one .py per schema).
# The worker_v2 directory name is intentional while the frozen V1 worker
# contract still exists elsewhere in the tree.
$CODEGEN \
    --input schemas/ipc/worker_v2/ \
    --output python/batchalign/generated/worker_v2/ \
    "${COMMON_OPTS[@]}" 2>&1 || {
    echo "WARNING: datamodel-codegen failed for worker_v2."
    echo "Install with: uvx --from datamodel-code-generator datamodel-codegen"
}

# Batch item types
$CODEGEN \
    --input schemas/ipc/batch_items/ \
    --output python/batchalign/generated/batch_items/ \
    "${COMMON_OPTS[@]}" 2>&1 || {
    echo "WARNING: datamodel-codegen failed for batch_items."
}

echo "Done. Generated:"
echo "  schemas/ipc/          — JSON Schema (67 types)"
echo "  python/batchalign/generated/ — Python Pydantic models"
echo ""
echo "Next: migrate hand-written Python models to import from python/batchalign/generated/"
