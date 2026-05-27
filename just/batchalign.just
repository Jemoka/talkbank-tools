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
# Standalone daemon binary:
#   `just batchalign daemonapp` produces python/target/pyapp/bin/daemonapp,
#   a self-contained executable bundling CPython + the wheel + the [api]
#   extra via PyApp (https://ofek.dev/pyapp/). The binary IS the daemon —
#   running it boots `batchalign3 daemon` directly. See
#   `bazel/python/pyapp_build.sh` for the build mechanics + env-var
#   contract; the pyapp version pin lives in pyproject.toml's
#   [tool.batchalign.pinned_tools].
#
# Source of truth: python/pyproject.toml owns the wheel version, deps,
# all optional-deps groups, classifiers, scripts.
#
# Proto codegen (Rust → Python wire types). NO `gen-proto` RECIPE.
#   The pydantic-v2 `_proto_generated.py` is a Bazel-managed artifact,
#   regenerated from `crates/batchalign/batchalign-core/src/proto/*.rs`
#   via the `//python/batchalign/_core:_proto_generated_py` genrule any
#   time a relevant Rust file changes. Every recipe below pulls it in
#   transparently as a dep:
#     - `build`/`test` reach it via `//python/batchalign:batchalign_lib`
#       (which depends on `//python/batchalign/_core:proto`)
#     - `wheel`/`multiwheel` stage it into the source tree just before
#       `maturin build` packages (see `bazel/python/maturin_build.sh`)
#     - Pure `cd python && uv run maturin develop` (no Bazel) gets it
#       via `crates/batchalign/batchalign-engine/build.rs`, which walks
#       the in-process `inventory::iter::<ProtoSchemaEntry>` and shells
#       out to `bazel/python/codegen_proto.py`.
#   Adding a new wire type means: define the struct, call
#   `register_proto_schema!(NewThing)` next to it in the proto module —
#   nothing else.

set shell := ["bash", "-c"]
set positional-arguments := true

# Default: list available recipes.
default:
    @just --list --justfile {{ justfile() }}

# Regenerate the Python lockfiles from pyproject.toml IF pyproject changed.
#
# `uv.lock` and `requirements.lock.txt` are derived artifacts of
# `python/pyproject.toml`; editing deps there must propagate to both before
# any Bazel build, or the py_binary runs against a stale dependency set (e.g.
# a newly-added `nltk` would be missing at runtime). This is mtime-guarded so
# it's a no-op once the locks are current — regenerating `requirements.lock.txt`
# bumps its mtime past pyproject's, so steady-state `cli`/`build` calls skip it.
_synclock:
    @if [ '{{ justfile_directory() }}/python/pyproject.toml' -nt '{{ justfile_directory() }}/python/requirements.lock.txt' ]; then \
        echo "pyproject.toml changed → relocking (uv.lock + requirements.lock.txt)…"; \
        (cd '{{ justfile_directory() }}/python' && uv lock) \
        && bazel run //python:requirements; \
    fi

# Build every Batchalign Bazel target (Rust + py_binary + wheel).
build profile="release": _synclock
    bazel build --config={{ if profile == "release" { "release" } else { "dev" } }} \
        //crates/batchalign/... //python/batchalign/...

# Run every Batchalign Bazel test target. Includes
# //python:uv_lock_check which fails if pyproject.toml drifted from
# uv.lock (`cd python && uv lock` to fix).
test profile="release":
    bazel test --config={{ if profile == "release" { "release" } else { "dev" } }} \
        //crates/batchalign/... //python/batchalign/... //python:uv_lock_check

# Run the Python `batchalign3` CLI via py_binary.
#
# `bazel run` cd's to its execroot before invoking the binary, which breaks
# relative-path args (e.g. `../talkbank-alignment/foo`). `--run_under` is
# prepended as a shell command, so cd-ing back to the user's invocation
# directory restores the expected cwd semantics for path arguments.
cli *args: _synclock
    bazel run --run_under="cd '{{ invocation_directory() }}' && " \
        //python/batchalign -- "$@"

# Pytest via py_test.
pytest *args:
    bazel test //python/batchalign:pytest -- "$@"

# mypy + ruff.
lint:
    bazel run //python/batchalign:lint

# ---------------------------------------------------------------------------
# Wheel build.
#
# `wheel` produces a `cp310-abi3-<host-platform>` wheel that installs on
# any CPython >= 3.10 (stable ABI). Pure-Rust deps -- the wheel records
# no dynamic library paths beyond the OS base libraries.
#
# Cross-platform builds are CI-only: `.github/workflows/publish-pypi.yml`
# fans out to one native runner per platform. Do not try to cross-build
# from your laptop -- there's no recipe for it, by design. If you need a
# wheel for a platform you can't run natively, dispatch the CI workflow.
# ---------------------------------------------------------------------------

# Wheel for the host platform.
wheel:
    bazel run -c opt //python/batchalign:wheel

# Upload every wheel under python/target/wheels/ to PyPI via twine.
# Requires PYPI_TOKEN in the environment (or ~/.pypirc credentials).
publish:
    bazel run //python/batchalign:publish

# Standalone production daemon binary via PyApp.
#
# Output: python/target/pyapp/bin/daemonapp. Self-contained — bundles
# CPython at the pinned major.minor, the batchalign wheel, and the
# [api] extra (fastapi/uvicorn/sse-starlette). Run it directly to boot
# the HTTP daemon; no Python or batchalign install on the host.
#
# First build: ~minutes (cargo install pyapp + maturin build).
# Rebuilds: only the maturin step reruns if Rust sources changed.
#
# Override via PYAPP_PROFILE={release|debug}, PYAPP_PYTHON_VERSION=...,
# PYAPP_DISTRIBUTION_EMBED=0 (download Python on first run instead of
# embedding it -- smaller binary, needs network at first launch).
daemonapp:
    bazel run -c opt //python/batchalign:daemonapp

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
