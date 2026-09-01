# Batchalign — pyo3 Rust engine + Python wheel + `batchalign3` CLI.
#
# Batchalign build, test, CLI, and release recipes:
#   build [profile]   build every Bazel target for the product
#   test  [profile]   run every Bazel test target for the product
#   cli  *args        primary user-facing CLI for the product
#   versions          source-of-truth versions for this product
# Batchalign-specific:
#   pytest *args      run py_test (pytest under Bazel)
#   lint              mypy (+ ruff if present)
#   wheel             host-platform wheel into python/target/wheels/
#   sidecar           self-contained daemon binary via PyApp
#
# Profile is `release` (default) or `debug`. Maps to:
#   - Bazel: --config=release | --config=dev
#   - maturin: --release | (dev)
#
# Pinned tools (asserted at script entry by `bazel/python/hermeticity_guard.sh`):
#   uv, maturin, python, rustc — pins live in `python/pyproject.toml`
#   under [tool.batchalign.pinned_tools]. Update there + MODULE.bazel.
#
# Wheel artifact path:
#   python/target/wheels/batchalign-<version>-cp310-abi3-<host-plat>.whl
#
# Cross-platform wheel builds are CI-only via
# `.github/workflows/publish-pypi.yml` (5-runner matrix, OIDC trusted
# publishing). There is no local cross-build recipe and no local
# publish recipe — both invite the wrong mental model.
#
# Standalone daemon sidecar binary:
#   `just batchalign sidecar` produces python/target/pyapp/bin/sidecar,
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
#     - `wheel` stages it into the source tree just before maturin
#       packages (see `bazel/python/maturin_build.sh`)
#     - Pure `cd python && uv run maturin develop` (no Bazel) gets it
#       via `crates/batchalign/batchalign-engine/build.rs`, which walks
#       the in-process `inventory::iter::<ProtoSchemaEntry>` and shells
#       out to `bazel/python/codegen_proto.py`.
#   Adding a new wire type means: define the struct, call
#   `register_proto_schema!(NewThing)` next to it in the proto module —
#   nothing else.
#
# Dep-file reactivity is automatic via `tools/bazel` (the Bazelisk
# wrapper at the workspace root). Editing pyproject.toml regenerates
# uv.lock + requirements.lock.txt on the next Bazel invocation; editing
# Cargo.toml triggers crate_universe to re-resolve. No just recipe
# wraps this — the wrapper makes it transparent.

set shell := ["bash", "-c"]
set positional-arguments := true

# Use Bazel's host-adaptive parallelism for normal builds. Resource-constrained
# automation can set BATCHALIGN_BAZEL_JOBS without changing user defaults.
bazel_jobs := env_var_or_default("BATCHALIGN_BAZEL_JOBS", "auto")

# Default: list available recipes.
default:
    @just --list --justfile {{ justfile() }}

# Build every Batchalign Bazel target (Rust + py_binary + wheel + GUI sources).
# Note: the GUI bundle (`:bundle`) is reserved for `just batchalign gui` —
# `bazel build //apps/batchalign/...` only builds the frontend filegroup
# (sh_binary's `data` graph is materialized at `bazel run` time).
build profile="release":
    bazel build --jobs={{ bazel_jobs }} --config={{ if profile == "release" { "release" } else { "dev" } }} \
        //crates/batchalign/... //python/batchalign/... //apps/batchalign/...

# Run every Batchalign Rust test plus the Python pytest target. The engine
# tests are tagged manual to keep broad Bazel wildcards from rebuilding the
# extension, so the authoritative recipe names them explicitly. Naming the
# Python test explicitly keeps ordinary tests from building release-only
# non-test targets such as the maturin wheel artifact.
test profile="release":
    bazel test --jobs={{ bazel_jobs }} --config={{ if profile == "release" { "release" } else { "dev" } }} \
        //crates/batchalign/... \
        //crates/batchalign/batchalign-engine:batchalign_engine_unit_test \
        //crates/batchalign/batchalign-engine:cache_smoke \
        //crates/batchalign/batchalign-engine:pipeline_smoke \
        //python/batchalign:pytest

# Run the Python `batchalign3` CLI via py_binary.
#
# `bazel run` cd's to its execroot before invoking the binary, which breaks
# relative-path args (e.g. `../talkbank-alignment/foo`). `--run_under` is
# prepended as a shell command, so cd-ing back to the user's invocation
# directory restores the expected cwd semantics for path arguments.
cli *args:
    bazel run --jobs={{ bazel_jobs }} --run_under="cd '{{ invocation_directory() }}' && " \
        //python/batchalign -- "$@"

# Pytest via py_test.
pytest *args:
    test_args=(); \
    for arg in "$@"; do test_args+=(--test_arg="$arg"); done; \
    bazel test --jobs={{ bazel_jobs }} "${test_args[@]}" //python/batchalign:pytest

# mypy + ruff.
lint:
    bazel run --jobs={{ bazel_jobs }} //python/batchalign:lint

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
#
# Single target end-to-end: `:wheel` is a sh_binary that depends on
# the `:_wheel_artifact` genrule (one maturin run, Bazel-cached, also
# consumed by `:sidecar`) and copies the wheel into
# `python/target/wheels/<tagged-name>.whl`. Building both targets
# back-to-back doesn't double-invoke maturin.
wheel:
    bazel run --jobs={{ bazel_jobs }} -c opt //python/batchalign:wheel

# Standalone production daemon sidecar binary via PyApp.
#
# Output: bazel-bin/python/batchalign/sidecar. Self-contained —
# bundles CPython at the pinned major.minor, the batchalign wheel,
# and the [api] extra (fastapi/uvicorn/sse-starlette). Run it
# directly to boot the HTTP daemon; no Python or batchalign install
# on the host. Named "sidecar" because that's its deployment role
# (companion service next to the main CLI/wheel install, not a
# separate top-level app).
#
# All tools are Bazel-managed — pyapp source via @pyapp_src
# (git_repository, pinned by SHA); cargo + rustc via rules_rust's
# toolchain. Users only need `bazel` and `just` installed.
#
# First build: ~few minutes (cargo build of pyapp + maturin wheel).
# Incremental rebuilds: sub-second when nothing changed.
sidecar:
    bazel build --jobs={{ bazel_jobs }} -c opt //python/batchalign:sidecar

# Desktop GUI submodule.
#
#   just batchalign::gui::build [profile]   bundle .app/.msi/.AppImage (cargo tauri build via Bazel)
#   just batchalign::gui::dev               cargo tauri dev (hot reload, no Bazel)
#   just batchalign::gui::openapi           generate OpenAPI + capabilities JSON via Bazel
#   just batchalign::gui::test              Playwright end-to-end test against a real sidecar
#
# All three live in `just/batchalign-gui.just`. Discovery:
#   just --list batchalign::gui
mod gui "batchalign-gui.just"

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
