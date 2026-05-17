# Batchalign — pyo3 Rust engine + Python wheel + `batchalign3` CLI.
#
# Uniform recipe shape across products (batchalign + chatter):
#   build [profile]   build every Bazel target for the product
#   test  [profile]   run every Bazel test target for the product
#   cli  *args        primary user-facing CLI for the product
#   versions          source-of-truth versions for this product
# Batchalign-specific:
#   relock            regenerate python/requirements.lock.txt from pyproject.toml
#   pytest *args      run py_test (pytest under Bazel)
#   lint              mypy (+ ruff if present)
#   wheel             host-platform wheel
#   wheel-<platform>  per-platform wheel (5 platforms)
#   multiwheel        all platforms (host triples succeed locally; CI fans out)
#   publish           twine upload python/target/wheels/*.whl
#
# Profile is `release` (default) or `debug`. Maps to:
#   - Bazel: --config=release | --config=dev
#   - maturin: --release | (dev)
#
# Pinned tools (asserted at script entry by `bazel/python/hermeticity_guard.sh`):
#   uv, maturin, python, rustc — pins live in `python/pyproject.toml`
#   under [tool.batchalign.pinned_tools]. Update there + MODULE.bazel.
#
# Wheel artifact paths:
#   per-platform: python/target/wheels/batchalign-<version>-cp312-cp312-<plat>.whl
#
# Source of truth: python/pyproject.toml owns the wheel version, deps,
# all optional-deps groups, classifiers, scripts.

set shell := ["bash", "-c"]
set positional-arguments := true

# Default: list available recipes.
default:
    @just --list --justfile {{ justfile() }}

# Build every Batchalign Bazel target (Rust + py_binary + wheel).
build profile="release":
    bazel build --config={{ if profile == "release" { "release" } else { "dev" } }} \
        //crates/batchalign/... //python/batchalign/...

# Run every Batchalign Bazel test target. Includes
# //python:uv_lock_check which fails if pyproject.toml drifted from
# uv.lock (`cd python && uv lock` to fix).
test profile="release":
    bazel test --config={{ if profile == "release" { "release" } else { "dev" } }} \
        //crates/batchalign/... //python/batchalign/... //python:uv_lock_check

# Run the Python `batchalign3` CLI via py_binary.
cli *args:
    bazel run //python/batchalign -- "$@"

# Pytest via py_test.
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
    @root="{{ justfile_directory() }}"; \
    echo "Python wheel  (python/pyproject.toml [project].version):"; \
    awk -F'"' '/^version *= *"/ {print "  " $2; exit}' "$root/python/pyproject.toml"; \
    echo "Rust workspace (Cargo.toml [workspace.package].version):"; \
    awk -F'"' '/^\[workspace\.package\]/{f=1;next} f&&/^\[/{exit} f&&/^version *= *"/{print "  " $2;exit}' "$root/Cargo.toml"; \
    echo "batchalign-engine (crates/batchalign/batchalign-engine/Cargo.toml [package].version):"; \
    awk -F'"' '/^version *= *"/ {print "  " $2; exit}' "$root/crates/batchalign/batchalign-engine/Cargo.toml"; \
    echo "Pinned tools (python/pyproject.toml [tool.batchalign.pinned_tools]):"; \
    awk '/^\[tool\.batchalign\.pinned_tools\]/{f=1;next} f&&/^\[/{exit} f&&/=/{print "  " $0}' "$root/python/pyproject.toml"
