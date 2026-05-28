#!/usr/bin/env bash
# Build the standalone `sidecar` binary via PyApp.
#
# PyApp (https://ofek.dev/pyapp/) is a tiny Rust runtime that bootstraps
# a Python application from a wheel. The pattern is:
#
#   1. Build the batchalign wheel via maturin.
#   2. `cargo install pyapp` with `PYAPP_*` env vars pointing at that
#      wheel + the entry function we want invoked at runtime.
#   3. Cargo links the embedded wheel into the produced binary; the
#      binary unpacks + runs it on first invocation.
#
# This follows the same shell-out pattern as `maturin_build.sh`: the
# wheel build cannot be expressed inside Bazel's hermetic sandbox
# (maturin needs the host cargo + a writable target dir), so we live
# with the escape and use `hermeticity_guard.sh` to assert toolchain
# pins instead.
#
# Hermeticity considerations for the *additional* shell-out PyApp
# introduces:
#   - `cargo install pyapp --version <pin>` pins the published crate
#     version from crates.io. Combined with the rustc pin (already
#     guarded), the produced binary is reproducible.
#   - The wheel is built from the workspace, not fetched.
#   - The Python distribution PyApp embeds is itself version-pinned via
#     PYAPP_PYTHON_VERSION below (defaults to the workspace's pinned
#     Python major.minor).
#
# Profile selection follows Bazel's compilation_mode, propagated via the
# `$(COMPILATION_MODE)` make-variable in BUILD.bazel `args` (so users
# never set env vars themselves -- `bazel run -c opt|dbg|fastbuild ...`
# is the single source of truth):
#   opt        -> cargo install release      (default for bazel run)
#   dbg        -> cargo install --debug
#   fastbuild  -> cargo install --debug
# Escape hatch for tooling that bypasses Bazel: PYAPP_PROFILE={release|debug}
# overrides the propagated value.
#
# Hermetic C toolchain. Cargo's cc-rs picks up CC/CXX/AR/RANLIB from the
# environment; we point them at the toolchains_llvm-managed binaries that
# Bazel stages into the sh_binary's runfiles tree. The host /usr/bin/cc
# is never consulted, so a broken host clang / CommandLineTools state
# doesn't propagate into the wheel build. See MODULE.bazel
# `toolchains_llvm` block for the pin + cflag policy.
#
# Args (all passed by the sh_binary; users do not set env vars):
#   $1 = uv binary (@multitool//tools/uv)
#   $2 = path to the Bazel-built `_proto_generated.py` artifact
#        (same contract as maturin_build.sh -- the wheel build wants it
#        staged into the source tree).
#   $3 = Bazel compilation_mode ($(COMPILATION_MODE)): opt|dbg|fastbuild.
#   $4 = hermetic clang  (@llvm_toolchain_llvm//:clang)
#   $5 = hermetic ar     (@llvm_toolchain_llvm//:ar)
#   $6 = hermetic ranlib (@llvm_toolchain_llvm//:ranlib)
set -euo pipefail

UV="$1"; shift
PROTO_GENERATED="$1"; shift
COMPILATION_MODE="${1:-opt}"; shift || true
CC_RLOC="$1"; shift
AR_RLOC="$1"; shift
RANLIB_RLOC="$1"; shift

# shellcheck source=hermeticity_guard.sh
source "${BUILD_WORKSPACE_DIRECTORY}/bazel/python/hermeticity_guard.sh"
hermeticity_guard "$UV"
UV="$HERMETIC_UV"

# ---------------------------------------------------------------------------
# Resolve the C toolchain.
#
# On macOS we MUST use Xcode's bundled clang (via `xcrun -find clang`),
# not the toolchains_llvm clang shipped in runfiles. Reason: the macOS
# SDK headers reference clang language features whose grammar changes
# between major clang versions (`__sized_by`, `__deprecated_enum_msg`,
# `__kernel_ptr_semantics`, etc.). Apple ships SDK + clang as a matched
# pair inside Xcode; mixing the SDK from Xcode 26.x with an older
# upstream clang (toolchains_llvm 1.7.0 caps darwin-arm64 at LLVM
# 17.0.6 -- LLVM stopped publishing arm64-apple-darwin prebuilts) fails
# at SDK header parse time. There is no hermetic darwin-arm64 clang
# available that's new enough; we accept the host Xcode toolchain as
# the macOS reality.
#
# On Linux/Windows the toolchains_llvm clang is fully hermetic (LLVM
# and the system libc are independently versioned) and stays in use.
# shellcheck source=runfiles_resolve.sh
source "${BUILD_WORKSPACE_DIRECTORY}/bazel/python/runfiles_resolve.sh"
case "$(uname -s)" in
    Darwin)
        CC_ABS="$(xcrun -find clang 2>/dev/null || true)"
        CXX_ABS="$(xcrun -find clang++ 2>/dev/null || true)"
        AR_ABS="$(xcrun -find ar 2>/dev/null || true)"
        RANLIB_ABS="$(xcrun -find ranlib 2>/dev/null || true)"
        if [[ -z "$CC_ABS" || ! -x "$CC_ABS" ]]; then
            echo "pyapp_build.sh: xcrun could not find clang. Install Xcode and run" >&2
            echo "    sudo xcode-select -s /Applications/Xcode.app/Contents/Developer" >&2
            echo "  See CONTRIBUTING.md 'macOS: install Xcode' for the full setup." >&2
            exit 2
        fi
        toolchain_source="Xcode ($(dirname "$(dirname "$CC_ABS")"))"
        ;;
    *)
        CC_ABS="$(runfiles_resolve "$CC_RLOC")"
        CXX_ABS="$CC_ABS"
        AR_ABS="$(runfiles_resolve "$AR_RLOC")"
        RANLIB_ABS="$(runfiles_resolve "$RANLIB_RLOC")"
        toolchain_source="toolchains_llvm (runfiles)"
        ;;
esac
export CC="$CC_ABS"
export CXX="$CXX_ABS"
export AR="$AR_ABS"
export RANLIB="$RANLIB_ABS"
# cc-rs respects per-target overrides too; set them so the cross-compile
# code paths (MATURIN_TARGET) don't fall back to host probes.
host_triple="$(rustc -vV 2>/dev/null | sed -n 's/^host: //p')"
if [[ -n "$host_triple" ]]; then
    # macOS ships bash 3.2; the `${var^^}` uppercase syntax is bash 4+.
    triple_underscored="${host_triple//-/_}"
    triple_upper="$(printf '%s' "$triple_underscored" | tr '[:lower:]' '[:upper:]')"
    export "CC_${triple_underscored}=$CC_ABS"
    export "CXX_${triple_underscored}=$CXX_ABS"
    export "AR_${triple_underscored}=$AR_ABS"
    export "CARGO_TARGET_${triple_upper}_LINKER=$CC_ABS"
fi
echo "pyapp_build.sh: CC=$CC_ABS  source=$toolchain_source"

# ---------------------------------------------------------------------------
# Stage the generated proto module (same as maturin_build.sh -- the wheel
# build reads it from the source tree).
# ---------------------------------------------------------------------------
proto_dst="${BUILD_WORKSPACE_DIRECTORY}/python/batchalign/_core/_proto_generated.py"
if [[ ! -f "$PROTO_GENERATED" ]]; then
    echo "pyapp_build.sh: missing generated proto at $PROTO_GENERATED" >&2
    exit 2
fi
# Force-overwrite: a prior Bazel run can have left a read-only artifact
# at this path (Bazel outputs are mode 0444). `cp -f` removes the
# destination first when it can't be opened for writing.
rm -f "$proto_dst" 2>/dev/null || chmod +w "$proto_dst" 2>/dev/null || true
cp "$PROTO_GENERATED" "$proto_dst"
chmod +w "$proto_dst"

cd "$BUILD_WORKSPACE_DIRECTORY/python"

# cc-rs (libsqlite3-sys and friends) needs SDKROOT pointing at the
# macOS SDK so the hermetic clang can resolve <sys/proc.h>, <mach/...>
# and friends. Interactive Mac shells usually have SDKROOT set via the
# user's profile or xcrun shim; non-interactive subshells (Bazel run,
# CI runners, fresh terminals) often do not.
#
# Use `xcrun --sdk macosx --show-sdk-path`, NOT `xcrun --show-sdk-path`:
# the latter is sticky on whichever SDK xcrun saw first (often the CLT
# one) even after `xcode-select -s /Applications/Xcode.app/...`, while
# the explicit `--sdk macosx` always returns the SDK in the currently
# active developer dir. On hosts with the broken CLT 26.x bundle this
# matters -- the CLT SDK fails to compile its own headers but the
# Xcode SDK is fine.
if [[ "${SDKROOT:-}" == "" ]] && command -v xcrun >/dev/null 2>&1; then
    SDKROOT="$(xcrun --sdk macosx --show-sdk-path 2>/dev/null || true)"
    if [[ -n "$SDKROOT" ]]; then
        export SDKROOT
        echo "pyapp_build.sh: exported SDKROOT=$SDKROOT"
    fi
fi

# ---------------------------------------------------------------------------
# Resolve pinned tool versions.
# ---------------------------------------------------------------------------
pyproject="${BUILD_WORKSPACE_DIRECTORY}/python/pyproject.toml"
pin_pyapp=$(sed -n "/^\[tool\.batchalign\.pinned_tools\]/,/^\[/p" "$pyproject" \
    | sed -n 's/^pyapp[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' | head -1)
pin_python=$(sed -n "/^\[tool\.batchalign\.pinned_tools\]/,/^\[/p" "$pyproject" \
    | sed -n 's/^python[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' | head -1)
if [[ -z "$pin_pyapp" ]]; then
    echo "pyapp_build.sh: missing pyapp pin in [tool.batchalign.pinned_tools]" >&2
    exit 2
fi

# ---------------------------------------------------------------------------
# Step 1: build the wheel.
# ---------------------------------------------------------------------------
resolved_profile="${PYAPP_PROFILE:-$COMPILATION_MODE}"
case "$resolved_profile" in
    release|opt) maturin_flag=(--release); cargo_flag=() ;;
    debug|dbg|fastbuild) maturin_flag=(); cargo_flag=(--debug) ;;
    *) echo "pyapp_build.sh: unknown profile $resolved_profile (from \$(COMPILATION_MODE)=$COMPILATION_MODE)" >&2; exit 2 ;;
esac
echo "pyapp_build.sh: compilation_mode=$COMPILATION_MODE -> profile=$resolved_profile"


echo "pyapp_build.sh: building wheel via maturin..."
"$UV" run maturin build "${maturin_flag[@]+"${maturin_flag[@]}"}" --out target/wheels

# Locate the freshly-built wheel. maturin doesn't print the artifact
# path on a single line we can parse, so glob the output dir.
wheel="$(ls -t target/wheels/batchalign-*.whl 2>/dev/null | head -1 || true)"
if [[ -z "$wheel" || ! -f "$wheel" ]]; then
    echo "pyapp_build.sh: maturin did not produce a wheel in target/wheels/" >&2
    exit 2
fi
wheel_abs="$(cd "$(dirname "$wheel")" && pwd)/$(basename "$wheel")"
echo "pyapp_build.sh: built $wheel_abs"

# ---------------------------------------------------------------------------
# Step 2: cargo install pyapp with env-var configuration.
#
# PyApp env vars (see https://ofek.dev/pyapp/latest/config/):
#   PYAPP_PROJECT_PATH     -- local wheel to embed (skips PyPI fetch)
#   PYAPP_EXEC_SPEC        -- "module:func" invoked at runtime
#   PYAPP_PYTHON_VERSION   -- pinned Python distribution
#   PYAPP_DISTRIBUTION_EMBED -- "1" -> ship a self-contained binary
#                               with the Python runtime baked in (no
#                               first-run network).
#   PYAPP_FULL_ISOLATION   -- "1" -> never share the unpack dir across
#                               users; safer for multi-tenant hosts.
# ---------------------------------------------------------------------------
out_dir="${BUILD_WORKSPACE_DIRECTORY}/python/target/pyapp"
rm -rf "$out_dir"
mkdir -p "$out_dir"

export PYAPP_PROJECT_PATH="$wheel_abs"
export PYAPP_EXEC_SPEC="batchalign.cli.daemon:run_pyapp_entry"
export PYAPP_PYTHON_VERSION="${PYAPP_PYTHON_VERSION:-$pin_python}"
export PYAPP_DISTRIBUTION_EMBED="${PYAPP_DISTRIBUTION_EMBED:-1}"
export PYAPP_FULL_ISOLATION="${PYAPP_FULL_ISOLATION:-1}"
# The bundled binary should install the [api] extra so uvicorn / fastapi
# / sse-starlette are available at runtime without the operator picking
# the right extra. PyApp passes this string straight to pip.
export PYAPP_PROJECT_DEPENDENCY_FILE=""
export PYAPP_PIP_EXTRA_ARGS="${PYAPP_PIP_EXTRA_ARGS:-}"
export PYAPP_PROJECT_FEATURES="api"

echo "pyapp_build.sh: cargo install pyapp@$pin_pyapp"
echo "  PYAPP_PROJECT_PATH=$PYAPP_PROJECT_PATH"
echo "  PYAPP_EXEC_SPEC=$PYAPP_EXEC_SPEC"
echo "  PYAPP_PYTHON_VERSION=$PYAPP_PYTHON_VERSION"
echo "  PYAPP_DISTRIBUTION_EMBED=$PYAPP_DISTRIBUTION_EMBED"

cargo install pyapp \
    --version "$pin_pyapp" \
    --force \
    --root "$out_dir" \
    "${cargo_flag[@]+"${cargo_flag[@]}"}"

# cargo places the binary at <root>/bin/pyapp; rename to sidecar so
# the produced artifact reflects its deployment role (the batchalign
# HTTP daemon sidecar that ships alongside the main wheel/CLI).
final="$out_dir/bin/sidecar"
mv "$out_dir/bin/pyapp" "$final"

echo "pyapp_build.sh: produced $final"
ls -lh "$final"

# Print the path on the last line so callers / CI scripts can capture it.
echo "SIDECAR_PATH=$final"
