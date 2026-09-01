#!/bin/sh

set -eu

if ! command -v uv >/dev/null 2>&1; then
    echo "Installing uv..."
    curl -LsSf https://astral.sh/uv/install.sh | sh
    PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
    export PATH
fi

if ! command -v uv >/dev/null 2>&1; then
    echo "uv was installed, but it is not available on PATH." >&2
    echo "Start a new shell and rerun this script." >&2
    exit 1
fi

if uv tool list --show-extras | grep -Eq '^batchalign v.*\[extras: ([^]]*, )?all(, [^]]*)?\]'; then
    echo "Upgrading batchalign[all]..."
    uv tool install --upgrade --python=3.11 --prerelease=allow 'batchalign[all]'
else
    echo "Installing batchalign[all]..."
    uv tool install --python=3.11 --prerelease=allow 'batchalign[all]'
fi

echo "Batchalign is ready."
