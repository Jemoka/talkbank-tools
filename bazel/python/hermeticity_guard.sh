#!/usr/bin/env bash
# Hermeticity guard for the maturin shell-out path.
#
# The wheel + dev-install path leaves Bazel's hermetic sandbox: maturin
# invokes the host cargo + rustc, uv resolves against the host network,
# and the resulting cdylib links against the host CPython framework.
# This guard asserts that the live tools match the pins recorded in
# `python/pyproject.toml [tool.batchalign.pinned_tools]` before any of
# those tools run. A pin mismatch fails loudly with an actionable message
# instead of producing a silently-divergent artifact.
#
# Usage (from a sibling wrapper script):
#     source "$(dirname "$0")/hermeticity_guard.sh"
#     hermeticity_guard "$UV"   # the multitool-provided uv binary
#
# Assertions:
#   - uv      version exactly matches `pinned_tools.uv`
#   - maturin version exactly matches `pinned_tools.maturin`
#   - python  major.minor matches `pinned_tools.python`
#   - rustc   version exactly matches `pinned_tools.rust`
#
# Skipping a check: set HERMETICITY_GUARD_ALLOW={uv,maturin,python,rustc}
# in the environment. Use sparingly and only in CI when a release runner
# can't match a pinned patch.

set -euo pipefail

# ---------------------------------------------------------------------------
# Environment scrubbing.
#
# The maturin shell-out invokes the host cargo, which in turn invokes the
# host C compiler. Both honor a long list of env vars that, if leaked
# from the developer's shell, silently change the build:
#
#   CC / CXX / AR / RANLIB          — compiler binaries
#   CFLAGS / CXXFLAGS / CPPFLAGS    — compile flags
#   LDFLAGS                         — link flags
#   DYLD_LIBRARY_PATH (macOS)
#   LD_LIBRARY_PATH (Linux)         — dynamic-loader search
#   LIBRARY_PATH / CPATH            — static linker / preprocessor search
#   PKG_CONFIG_PATH                 — pkg-config search
#   RUSTFLAGS / RUSTC / RUSTC_WRAPPER — rustc behavior
#   CARGO_TARGET_DIR                — output redirection
#   OPENSSL_DIR / PROTOC            — system-lib pointers
#
# A developer who has these set for some unrelated project ("oh I need
# DYLD_LIBRARY_PATH for that Conda env") will produce wheels that work
# locally and fail in CI. Scrub them on entry, then pin CC/CXX to a
# known-stable system compiler so the cdylib's C-side links the same way
# on every machine.
# ---------------------------------------------------------------------------
hermeticity_scrub_env() {
    unset CC CXX AR RANLIB \
          CFLAGS CXXFLAGS CPPFLAGS LDFLAGS \
          DYLD_LIBRARY_PATH LD_LIBRARY_PATH LIBRARY_PATH CPATH \
          PKG_CONFIG_PATH \
          RUSTFLAGS RUSTC RUSTC_WRAPPER \
          CARGO_TARGET_DIR \
          OPENSSL_DIR PROTOC \
          MACOSX_DEPLOYMENT_TARGET

    case "$(uname -s)" in
        Darwin)
            export CC=clang
            export CXX=clang++
            # Match `.bazelrc build:macos --macos_minimum_os=12.0`.
            export MACOSX_DEPLOYMENT_TARGET=12.0
            ;;
        Linux)
            export CC=gcc
            export CXX=g++
            ;;
        *)
            # Windows CI cells configure their own toolchain via the
            # GH Actions matrix; nothing to do locally.
            ;;
    esac
}

_guard_pin() {
    # Extract a single pin from pyproject.toml without requiring `tomllib`.
    # The file is structured enough that a sed line-grep is reliable here.
    local key="$1" pyproject="$2"
    sed -n "/^\[tool\.batchalign\.pinned_tools\]/,/^\[/p" "$pyproject" \
        | sed -n "s/^${key}[[:space:]]*=[[:space:]]*\"\([^\"]*\)\".*/\1/p" \
        | head -1
}

_guard_skip() {
    case ",${HERMETICITY_GUARD_ALLOW:-}," in
        *",$1,"*) return 0 ;;
        *) return 1 ;;
    esac
}

hermeticity_guard() {
    local uv="$1"
    local pyproject="${BUILD_WORKSPACE_DIRECTORY}/python/pyproject.toml"
    if [[ ! -f "$pyproject" ]]; then
        echo "hermeticity_guard: cannot find $pyproject" >&2
        return 2
    fi

    # Scrub shell-leaked env *before* the version probes — the probes
    # themselves shell out to uv/python/rustc and shouldn't be running
    # against a corrupted environment either.
    hermeticity_scrub_env

    local pin_uv pin_maturin pin_python pin_rust
    pin_uv=$(_guard_pin uv "$pyproject")
    pin_maturin=$(_guard_pin maturin "$pyproject")
    pin_python=$(_guard_pin python "$pyproject")
    pin_rust=$(_guard_pin rust "$pyproject")

    local fail=0

    # uv: version banner is `uv 0.5.18 (abc123 2024-12-30)`.
    if ! _guard_skip uv; then
        local live_uv
        live_uv="$("$uv" --version 2>/dev/null | awk '{print $2}')"
        if [[ "$live_uv" != "$pin_uv" ]]; then
            echo "hermeticity_guard: uv version mismatch (pinned=$pin_uv, live=$live_uv)" >&2
            echo "  fix: update MODULE.bazel uv.toolchain(uv_version=...) to match, then bazel sync" >&2
            fail=1
        fi
    fi

    # maturin: invoked through uv to honor pyproject's pinned version
    # (uv installs maturin from [build-system].requires == 1.7.4).
    if ! _guard_skip maturin; then
        local live_maturin
        live_maturin="$(cd "$BUILD_WORKSPACE_DIRECTORY/python" \
            && "$uv" run --quiet maturin --version 2>/dev/null \
            | awk '{print $2}')"
        if [[ "$live_maturin" != "$pin_maturin" ]]; then
            echo "hermeticity_guard: maturin version mismatch (pinned=$pin_maturin, live=${live_maturin:-<missing>})" >&2
            echo "  fix: update pyproject.toml [build-system].requires AND [tool.batchalign.pinned_tools].maturin" >&2
            fail=1
        fi
    fi

    # python: major.minor only (patch level is CI-controlled).
    if ! _guard_skip python; then
        local live_python
        live_python="$(cd "$BUILD_WORKSPACE_DIRECTORY/python" \
            && "$uv" run --quiet python -c 'import sys; print(f"{sys.version_info.major}.{sys.version_info.minor}")' 2>/dev/null)"
        if [[ "$live_python" != "$pin_python" ]]; then
            echo "hermeticity_guard: python version mismatch (pinned=$pin_python, live=${live_python:-<missing>})" >&2
            echo "  fix: update MODULE.bazel python.toolchain(python_version=...) AND .bazelrc python_version setting" >&2
            fail=1
        fi
    fi

    # rustc: required for the cargo invocation maturin makes.
    if ! _guard_skip rustc; then
        local live_rust
        live_rust="$(rustc --version 2>/dev/null | awk '{print $2}')"
        if [[ "$live_rust" != "$pin_rust" ]]; then
            echo "hermeticity_guard: rustc version mismatch (pinned=$pin_rust, live=${live_rust:-<missing>})" >&2
            echo "  fix: update MODULE.bazel rust.toolchain(versions=[...]) AND rust-toolchain.toml" >&2
            fail=1
        fi
    fi

    if (( fail )); then
        echo "hermeticity_guard: refusing to proceed; bypass individual checks via HERMETICITY_GUARD_ALLOW={uv,maturin,python,rustc}" >&2
        return 1
    fi
}
