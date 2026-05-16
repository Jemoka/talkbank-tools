# Batchalign — Rust engine, Python wheel (via maturin), CLI.
#
# Build profile: `release` (default) or `debug`. Maps to:
#   - Bazel: --config=release | --config=dev
#   - maturin: --release | (dev)
#
# Pinned tools (asserted at script entry by `bazel/python/hermeticity_guard.sh`):
#   uv, maturin, python, rustc — pins live in `python/pyproject.toml`
#   under [tool.batchalign.pinned_tools]. Update there + MODULE.bazel.
#
# Wheel artifact paths:
#   per-platform: python/target/wheels/batchalign-<version>-cp312-cp312-<plat>.whl
#   universal:    python/target/wheels/batchalign-<version>-py3-none-any.whl  (n/a — pyo3 wheel)
#
# Source of truth: python/pyproject.toml owns the wheel version, deps,
# all optional-deps groups, classifiers, scripts. Bazel does not
# duplicate that metadata.

set shell := ["bash", "-c"]
set positional-arguments := true

# Default: list available recipes.
default:
    @just --justfile {{ justfile() }} --list

# Build every Batchalign Bazel target (Rust + sh_binary wrappers).
build profile="release":
    bazel build --config={{ if profile == "release" { "release" } else { "dev" } }} \
        //crates/batchalign/... //python/batchalign/...

# Run every Batchalign Bazel test target.
test profile="release":
    bazel test --config={{ if profile == "release" { "release" } else { "dev" } }} \
        //crates/batchalign/... //python/batchalign/...

# Run the Python `batchalign3` CLI. py_binary; Bazel builds the cdylib
# natively via the engine `rust_shared_library` and runs `python -m
# batchalign` against it — no maturin, no `:develop` round-trip.
cli *args:
    bazel run //python/batchalign -- "$@"

# Lockfile regen (`pip_compile` reads pyproject.toml → requirements.lock.txt).
# Run after editing `[project] dependencies` in pyproject.toml; CI's
# `//python:requirements_test` gates drift.
relock:
    bazel run //python:requirements

# Pytest. py_test; Bazel builds the cdylib + Python deps, then runs.
pytest *args:
    bazel test //python/batchalign:pytest -- "$@"

# mypy + ruff.
lint:
    bazel run //python/batchalign:lint

# ---------------------------------------------------------------------------
# Wheel builds. Each target invokes maturin with --target <triple> so the
# wheel is tagged correctly. CI's matrix runs one cell per platform; locally,
# only the host-matching target succeeds without a cross-compile sysroot
# (use cibuildwheel / GitHub Actions for the full matrix).
# ---------------------------------------------------------------------------

# Wheel for the host platform (no explicit target triple).
wheel:
    MATURIN_PROFILE=release bazel run //python/batchalign:wheel

# Apple Silicon (M1/M2/M3).
wheel-macos-arm64:
    MATURIN_PROFILE=release MATURIN_TARGET=aarch64-apple-darwin \
        bazel run //python/batchalign:wheel

# Intel macOS.
wheel-macos-x86_64:
    MATURIN_PROFILE=release MATURIN_TARGET=x86_64-apple-darwin \
        bazel run //python/batchalign:wheel

# Linux glibc x86_64 (manylinux retagging happens at release time via auditwheel).
wheel-linux-x86_64:
    MATURIN_PROFILE=release MATURIN_TARGET=x86_64-unknown-linux-gnu \
        bazel run //python/batchalign:wheel

# Linux ARM64.
wheel-linux-aarch64:
    MATURIN_PROFILE=release MATURIN_TARGET=aarch64-unknown-linux-gnu \
        bazel run //python/batchalign:wheel

# Windows x86_64.
wheel-windows-x86_64:
    MATURIN_PROFILE=release MATURIN_TARGET=x86_64-pc-windows-msvc \
        bazel run //python/batchalign:wheel

# Build every supported wheel on the host. Only the host-matching triple
# succeeds without cross-compile sysroots installed; the rest are best
# delegated to the CI matrix (`.github/workflows/wheels.yml`).
multiwheel:
    @echo "→ Building wheels for all supported platforms (host-only triples will succeed)…"
    @echo "  Output: python/target/wheels/"
    -just batchalign wheel-macos-arm64
    -just batchalign wheel-macos-x86_64
    -just batchalign wheel-linux-x86_64
    -just batchalign wheel-linux-aarch64
    -just batchalign wheel-windows-x86_64
    @echo "→ Built wheels:"
    @ls -lh python/target/wheels/ 2>/dev/null || echo "  (no wheels produced)"

# Upload every wheel under python/target/wheels/ to PyPI via twine.
# Requires PYPI_TOKEN in the environment (or ~/.pypirc credentials).
publish:
    bazel run //python/batchalign:publish

# Print the source-of-truth versions for human inspection.
versions:
    @echo "Python wheel  (python/pyproject.toml [project].version):"
    @awk -F'"' '/^version *= *"/ {print "  " $2; exit}' python/pyproject.toml
    @echo "Rust workspace (Cargo.toml [workspace.package].version):"
    @awk -F'"' '/^\[workspace.package\]/,/^\[/ {if (/^version *= *"/) {print "  " $2; exit}}' Cargo.toml
    @echo "batchalign-engine (Cargo.toml override):"
    @awk -F'"' '/^version *= *"/ {print "  " $2; exit}' crates/batchalign/batchalign-engine/Cargo.toml
    @echo "Pinned tools (python/pyproject.toml [tool.batchalign.pinned_tools]):"
    @awk '/^\[tool.batchalign.pinned_tools\]/,/^\[/ {if (/=/) print "  " $0}' python/pyproject.toml | grep -v '^\['
