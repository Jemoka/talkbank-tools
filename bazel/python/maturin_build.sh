#!/usr/bin/env bash
# Build the batchalign wheel via maturin.
#
# Wheel builds use the release profile configured in pyproject.toml. Editable
# installs use its separate editable-profile setting so local development stays
# fast without allowing a debug-profile artifact into a release workflow.
#
# Platform targeting: set MATURIN_TARGET to a Rust target triple to
# cross-compile (e.g. `MATURIN_TARGET=aarch64-apple-darwin`). Defaults to
# the host triple. Used by the justfile's per-platform wheel recipes.
#
# Hermeticity:
#   - The guard script asserts uv/maturin/python/rustc versions match
#     the pins in pyproject.toml before any host-tool invocation.
#   - Linux release wheels use the lock-pinned Zig manylinux sysroot. macOS
#     uses Xcode's clang paired with its SDK, and Windows uses Visual Studio.
#
# Args (all passed by the sh_binary; users do not set env vars):
#   $1 = uv binary                                    (@multitool//tools/uv)
#   $2 = path to the Bazel-built `_proto_generated.py` artifact
#   $3 = C toolchain mode: `llvm` or `host`
#   $4..$6 = LLVM clang/ar/ranlib paths when mode is `llvm`
set -euo pipefail
UV="$1"; shift
PROTO_GENERATED="$1"; shift
TOOLCHAIN_MODE="${1:-}"; shift || true
case "$TOOLCHAIN_MODE" in
    llvm)
        if [[ $# -lt 3 ]]; then
            echo "maturin_build.sh: llvm mode requires clang, ar, and ranlib paths" >&2
            exit 2
        fi
        CC_RLOC="$1"; shift
        AR_RLOC="$1"; shift
        RANLIB_RLOC="$1"; shift
        ;;
    host) ;;
    *)
        echo "maturin_build.sh: unknown C toolchain mode '$TOOLCHAIN_MODE'" >&2
        exit 2
        ;;
esac

# shellcheck source=hermeticity_guard.sh
source "${BUILD_WORKSPACE_DIRECTORY}/bazel/python/hermeticity_guard.sh"
hermeticity_guard "$UV"
UV="$HERMETIC_UV"

# C toolchain wiring -- see pyapp_build.sh header for the full rationale.
# macOS uses Xcode's clang paired with its SDK. Linux resolves the Bazel LLVM
# inputs supplied by the shared rule but gives compilation/linking ownership to
# Zig's manylinux sysroot. Windows uses the Visual Studio host toolchain.
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
        use_zig=0
        platform_flags=()
        ;;
    MINGW*|MSYS*|CYGWIN*)
        if [[ "$TOOLCHAIN_MODE" != "host" ]]; then
            echo "maturin_build.sh: Windows requires host C toolchain mode" >&2
            exit 2
        fi
        toolchain_source="Visual Studio host"
        use_zig=0
        platform_flags=()
        ;;
    *)
        if [[ "$TOOLCHAIN_MODE" != "llvm" ]]; then
            echo "maturin_build.sh: Linux requires llvm C toolchain mode" >&2
            exit 2
        fi
        CC_ABS="$(runfiles_resolve "$CC_RLOC")"
        CXX_ABS="$CC_ABS"
        AR_ABS="$(runfiles_resolve "$AR_RLOC")"
        RANLIB_ABS="$(runfiles_resolve "$RANLIB_RLOC")"
        toolchain_source="zig (manylinux_2_28 sysroot)"
        use_zig=1
        # Hosted Ubuntu runners expose their host glibc to native C builds.
        # Zig supplies the requested older sysroot so the wheel cannot silently
        # regress to the runner's manylinux_2_38 compatibility floor.
        platform_flags=(--compatibility manylinux_2_28 --zig)
        ;;
esac
# Bazel's --jobs limit does not propagate into the Cargo process spawned by
# maturin. Keep that nested build serialized by default so a wheel action
# cannot fan out into one rustc per CPU and compete with the Bazel server for
# memory. Release automation may opt into a larger, explicit budget.
export CARGO_BUILD_JOBS="${BATCHALIGN_CARGO_JOBS:-${CARGO_BUILD_JOBS:-1}}"
if [[ -n "${CC_ABS:-}" && "$use_zig" != "1" ]]; then
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

echo "maturin_build.sh: C toolchain=$toolchain_source  cargo_jobs=$CARGO_BUILD_JOBS"

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
# sh_binary `data` dep promised it to be. Force-overwrite: a prior
# Bazel run can leave a read-only artifact (mode 0444) at this path,
# in which case `cp` would fail with "Permission denied"; remove or
# chmod the destination first so the copy can write.
proto_dst="${BUILD_WORKSPACE_DIRECTORY}/python/batchalign/_core/_proto_generated.py"
if [[ ! -f "$PROTO_GENERATED" ]]; then
    echo "maturin_build.sh: missing generated proto at $PROTO_GENERATED" >&2
    echo "Did the //python/batchalign/_core:_proto_generated_py genrule run?" >&2
    exit 2
fi
rm -f "$proto_dst" 2>/dev/null || chmod +w "$proto_dst" 2>/dev/null || true
cp "$PROTO_GENERATED" "$proto_dst"
chmod +w "$proto_dst"
echo "maturin_build.sh: staged $proto_dst from $PROTO_GENERATED"

# Remove extensions left by an earlier editable install. They are build
# products, not Python-package inputs; retaining one makes maturin add the
# same module once from the source tree and once from the current Cargo build.
native_module_dir="${BUILD_WORKSPACE_DIRECTORY}/python/batchalign/_core"
rm -f \
    "$native_module_dir/_core.pyd" "$native_module_dir"/_core.*.pyd \
    "$native_module_dir/_core.so" "$native_module_dir"/_core.*.so \
    "$native_module_dir/_core.dylib" "$native_module_dir"/_core.*.dylib \
    "$native_module_dir/_core.dll" "$native_module_dir"/_core.*.dll

cd "$BUILD_WORKSPACE_DIRECTORY/python"

echo "maturin_build.sh: wheel profile=release (configured by pyproject.toml)"

target_flag=()
if [[ -n "${MATURIN_TARGET:-}" ]]; then
    target_flag=(--target "$MATURIN_TARGET")
elif [[ "$use_zig" == "1" ]]; then
    host_triple="$(rustc -vV | sed -n 's/^host: //p')"
    if [[ -z "$host_triple" ]]; then
        echo "maturin_build.sh: rustc did not report a host target" >&2
        exit 2
    fi
    # cargo-zigbuild needs an explicit target even when target == host.
    target_flag=(--target "$host_triple")
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
    "${target_flag[@]+"${target_flag[@]}"}" \
    "${platform_flags[@]+"${platform_flags[@]}"}" \
    --out target/wheels \
    "$@"

ls -lh target/wheels/ || true
