#!/usr/bin/env bash
# Build the batchalign wheel via maturin.
#
# Profile selection follows Bazel's compilation_mode, propagated via the
# `$(COMPILATION_MODE)` make-variable in BUILD.bazel `args` so users
# never set env vars themselves -- `bazel run -c opt|dbg|fastbuild ...`
# is the single source of truth. MATURIN_PROFILE remains as an escape
# hatch for direct (non-Bazel) script invocations.
#
# Platform targeting: set MATURIN_TARGET to a Rust target triple to
# cross-compile (e.g. `MATURIN_TARGET=aarch64-apple-darwin`). Defaults to
# the host triple. Used by the justfile's per-platform wheel recipes.
#
# Hermeticity:
#   - The guard script asserts uv/maturin/python/rustc versions match
#     the pins in pyproject.toml before any host-tool invocation.
#   - CC/CXX/AR/RANLIB are pointed at toolchains_llvm-managed binaries
#     staged into the sh_binary's runfiles (see MODULE.bazel), so cargo's
#     cc-rs never touches /usr/bin/cc on the host. The SDK still comes
#     from xcrun on macOS; the toolchain cflag policy in MODULE.bazel
#     mirrors the SDK workaround into the env below.
#
# Args (all passed by the sh_binary; users do not set env vars):
#   $1 = uv binary                                    (@multitool//tools/uv)
#   $2 = path to the Bazel-built `_proto_generated.py` artifact
#   $3 = Bazel compilation_mode ($(COMPILATION_MODE)): opt|dbg|fastbuild
#   $4 = hermetic clang   (@llvm_toolchain_llvm//:clang)
#   $5 = hermetic ar      (@llvm_toolchain_llvm//:ar)
#   $6 = hermetic ranlib  (@llvm_toolchain_llvm//:ranlib)
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

# C toolchain wiring -- see pyapp_build.sh header for the full rationale.
# macOS: use Xcode's bundled clang (matched to its SDK). Linux/Windows:
# use the hermetic toolchains_llvm clang shipped in runfiles.
# shellcheck source=runfiles_resolve.sh
source "${BUILD_WORKSPACE_DIRECTORY}/bazel/python/runfiles_resolve.sh"
case "$(uname -s)" in
    Darwin)
        CC_ABS="$(xcrun -find clang 2>/dev/null || true)"
        CXX_ABS="$(xcrun -find clang++ 2>/dev/null || true)"
        AR_ABS="$(xcrun -find ar 2>/dev/null || true)"
        RANLIB_ABS="$(xcrun -find ranlib 2>/dev/null || true)"
        if [[ -z "$CC_ABS" || ! -x "$CC_ABS" ]]; then
            echo "maturin_build.sh: xcrun could not find clang. Install Xcode" >&2
            echo "  Command Line Tools (\`xcode-select --install\`); for the" >&2
            echo "  wheel path CLT is sufficient. If you later hit SDK-header" >&2
            echo "  parse errors (__kernel_ptr_semantics, __sized_by, fixpt_t)," >&2
            echo "  install full Xcode and point at it via:" >&2
            echo "    sudo xcode-select -s /Applications/Xcode.app/Contents/Developer" >&2
            echo "  See CONTRIBUTING.md 'macOS host prereqs' for the full setup." >&2
            exit 2
        fi
        toolchain_source="Xcode"
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
host_triple="$(rustc -vV 2>/dev/null | sed -n 's/^host: //p')"
if [[ -n "$host_triple" ]]; then
    triple_underscored="${host_triple//-/_}"
    triple_upper="$(printf '%s' "$triple_underscored" | tr '[:lower:]' '[:upper:]')"
    export "CC_${triple_underscored}=$CC_ABS"
    export "CXX_${triple_underscored}=$CXX_ABS"
    export "AR_${triple_underscored}=$AR_ABS"
    export "CARGO_TARGET_${triple_upper}_LINKER=$CC_ABS"
fi
if [[ "${BATCHALIGN_FORCE_DARWIN_SDK_WORKAROUND:-0}" == "1" ]]; then
    # Mismatched cross-compile fallback -- see pyapp_build.sh for rationale.
    flags=(-D__kernel_ptr_semantics= -D__kernel_data_semantics=)
    export CFLAGS="${CFLAGS:-} ${flags[*]}"
    export CXXFLAGS="${CXXFLAGS:-} ${flags[*]}"
fi
# Expose BSD types in macOS SDK -- see pyapp_build.sh for rationale.
if [[ "$(uname -s)" == "Darwin" ]]; then
    export CFLAGS="${CFLAGS:-} -D_DARWIN_C_SOURCE"
    export CXXFLAGS="${CXXFLAGS:-} -D_DARWIN_C_SOURCE"
fi

echo "maturin_build.sh: CC=$CC_ABS  source=$toolchain_source"

# SDKROOT: maturin shellouts run in a non-interactive subshell; xcrun
# fills in the gap when the user's profile hasn't exported SDKROOT.
# The SDK itself is still host-provided -- vendoring is a deliberate
# non-goal (Apple EULA / drift / surface area).
#
# `--sdk macosx` is mandatory: plain `xcrun --show-sdk-path` is sticky
# on whichever SDK xcrun saw first, often the CommandLineTools one
# even after `xcode-select -s /Applications/Xcode.app/...`. The
# `--sdk macosx` form always follows the active developer dir.
if [[ "${SDKROOT:-}" == "" ]] && command -v xcrun >/dev/null 2>&1; then
    SDKROOT="$(xcrun --sdk macosx --show-sdk-path 2>/dev/null || true)"
    [[ -n "$SDKROOT" ]] && export SDKROOT
fi

# Stage the autogenerated proto module. Resolve the Bazel-relative path
# against the runfiles tree; abort if the artifact isn't where the
# sh_binary `data` dep promised it to be.
proto_dst="${BUILD_WORKSPACE_DIRECTORY}/python/batchalign/_core/_proto_generated.py"
if [[ ! -f "$PROTO_GENERATED" ]]; then
    echo "maturin_build.sh: missing generated proto at $PROTO_GENERATED" >&2
    echo "Did the //python/batchalign/_core:_proto_generated_py genrule run?" >&2
    exit 2
fi
cp "$PROTO_GENERATED" "$proto_dst"
echo "maturin_build.sh: staged $proto_dst from $PROTO_GENERATED"

cd "$BUILD_WORKSPACE_DIRECTORY/python"

resolved_profile="${MATURIN_PROFILE:-$COMPILATION_MODE}"
case "$resolved_profile" in
    release|opt) profile_flag=(--release) ;;
    dev|dbg|fastbuild) profile_flag=() ;;
    *) echo "maturin_build.sh: unknown profile $resolved_profile (from \$(COMPILATION_MODE)=$COMPILATION_MODE)" >&2; exit 2 ;;
esac
echo "maturin_build.sh: compilation_mode=$COMPILATION_MODE -> profile=$resolved_profile"

target_flag=()
if [[ -n "${MATURIN_TARGET:-}" ]]; then
    target_flag=(--target "$MATURIN_TARGET")
fi

# `"${arr[@]+"${arr[@]}"}"` is the bash-set-u-safe way to splat a
# possibly-empty array — naked `"${arr[@]}"` trips `unbound variable`
# in bash 4.4+ when the array has zero elements.
# Do NOT pass `--manifest-path` on the command line: maturin treats that
# as "lone Cargo project" mode and derives the wheel name from Cargo.toml
# `[package].name` (which is `batchalign-engine`), bypassing the
# `[project].name = "batchalign"` declared in pyproject.toml. The
# `[tool.maturin].manifest-path` field in pyproject.toml supplies the
# Cargo.toml path while keeping pyproject as the metadata source of
# truth, which produces `batchalign-<version>-<tag>.whl`.
"$UV" run maturin build \
    "${profile_flag[@]+"${profile_flag[@]}"}" \
    "${target_flag[@]+"${target_flag[@]}"}" \
    --out target/wheels \
    "$@"

ls -lh target/wheels/ || true
