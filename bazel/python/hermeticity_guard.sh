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
# Environment scrubbing — narrow.
#
# The maturin shell-out invokes the host cargo, which invokes the host C
# compiler. Some shell env vars genuinely break reproducibility if
# leaked from a developer's session:
#
#   DYLD_LIBRARY_PATH / LD_LIBRARY_PATH — drag in unrelated dylibs at
#       runtime; the wheel works locally but fails in CI or in user
#       venvs that don't have those dirs.
#   RUSTFLAGS / RUSTC / RUSTC_WRAPPER — alter the rustc invocation
#       silently (e.g. an old `-C link-arg=...` from another project).
#   CARGO_TARGET_DIR — redirects the build output and confuses maturin's
#       wheel-find logic.
#   OPENSSL_DIR / PROTOC — system-lib pointers that, when present, take
#       precedence over what crate_universe set up.
#
# What we deliberately DO NOT scrub:
#   - CC / CXX / AR / RANLIB / CFLAGS / LDFLAGS / CPATH / LIBRARY_PATH /
#     SDKROOT / DEVELOPER_DIR / MACOSX_DEPLOYMENT_TARGET — these belong
#     to the host toolchain's normal configuration; clobbering them
#     breaks the macOS SDK's `<sys/proc.h>` (`u_quad_t`, `MAXCOMLEN`)
#     and other system headers that the user's shell already set up
#     correctly via xcode-select / homebrew / conda.
#   - PKG_CONFIG_PATH — sometimes needed for OpenSSL/zlib lookup on
#     macOS+homebrew layouts.
#
# Override per-invocation with HERMETICITY_GUARD_SKIP_SCRUB=1 if you're
# debugging a build that needs a specific shell var to leak through.
# ---------------------------------------------------------------------------
hermeticity_scrub_env() {
    if [[ "${HERMETICITY_GUARD_SKIP_SCRUB:-0}" = "1" ]]; then
        return 0
    fi
    unset DYLD_LIBRARY_PATH LD_LIBRARY_PATH \
          RUSTFLAGS RUSTC RUSTC_WRAPPER \
          CARGO_TARGET_DIR \
          OPENSSL_DIR PROTOC
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
    # The caller passes the multitool uv binary as $1; Bazel substitutes
    # a runfiles-relative path (e.g. `../rules_multitool~~...~uv/uv`),
    # which stops resolving the moment we `cd` away from runfiles.
    # Resolve to an absolute path here so the rest of this script — and
    # the caller's eventual `cd "$BUILD_WORKSPACE_DIRECTORY/python"` +
    # `"$UV" run ...` — keeps working.
    local uv_in="$1"
    local uv
    if [[ "$uv_in" = /* ]]; then
        uv="$uv_in"
    else
        uv="$(cd "$(dirname "$uv_in")" && pwd)/$(basename "$uv_in")"
    fi
    # Re-export so the caller (after `shift`) can use the absolute path.
    # The caller stores $1 in `UV` before calling us; we update that
    # binding by writing back to a well-known name they can read.
    HERMETIC_UV="$uv"

    local pyproject="${BUILD_WORKSPACE_DIRECTORY}/python/pyproject.toml"
    if [[ ! -f "$pyproject" ]]; then
        echo "hermeticity_guard: cannot find $pyproject" >&2
        return 2
    fi

    # Scrub shell-leaked env *before* the version probes — the probes
    # themselves shell out to uv/python/rustc and shouldn't be running
    # against a corrupted environment either.
    hermeticity_scrub_env

    # Ensure the uv-managed venv has the dev tools installed (maturin,
    # pytest, mypy, twine, ...). Do not install the project itself: uv's
    # editable install writes a native extension into the Python source tree.
    # On Windows that `_core.pyd` collides with the module maturin adds while
    # assembling the wheel. The wheel build below is the sole project build.
    if ! _guard_skip sync; then
        ( cd "$BUILD_WORKSPACE_DIRECTORY/python" \
            && "$uv" sync --extra dev --no-install-project --quiet ) || {
            echo "hermeticity_guard: failed to sync the uv venv ($BUILD_WORKSPACE_DIRECTORY/python)" >&2
            echo "  fix: run 'cd python && uv sync --extra dev --no-install-project' to surface uv's full error message" >&2
            return 1
        }
    fi

    local pin_uv pin_maturin pin_python pin_rust
    pin_uv=$(_guard_pin uv "$pyproject")
    pin_maturin=$(_guard_pin maturin "$pyproject")
    pin_python=$(_guard_pin python "$pyproject")
    pin_rust=$(_guard_pin rust "$pyproject")

    local fail=0

    # `||true` on every probe so a non-zero exit (pipefail + head closing
    # stdin, uv-run install failure, etc.) doesn't kill the script before
    # the explicit comparison runs. The comparison handles empty by
    # emitting a mismatch message with `<missing>`.

    # uv: version banner is `uv 0.5.18 (abc123 2024-12-30)`.
    if ! _guard_skip uv; then
        local live_uv
        live_uv="$("$uv" --version 2>/dev/null | awk '{print $2}' || true)"
        if [[ "$live_uv" != "$pin_uv" ]]; then
            echo "hermeticity_guard: uv version mismatch (pinned=$pin_uv, live=${live_uv:-<missing>})" >&2
            echo "  fix: update MODULE.bazel uv.toolchain(uv_version=...) to match, then bazel sync" >&2
            fail=1
        fi
    fi

    # maturin: invoked through uv to honor pyproject's pinned version
    # (uv installs maturin from [build-system].requires == 1.7.4).
    if ! _guard_skip maturin; then
        local live_maturin
        live_maturin="$( (cd "$BUILD_WORKSPACE_DIRECTORY/python" \
            && "$uv" run --quiet maturin --version 2>/dev/null) \
            | awk '{print $2}' || true)"
        if [[ "$live_maturin" != "$pin_maturin" ]]; then
            echo "hermeticity_guard: maturin version mismatch (pinned=$pin_maturin, live=${live_maturin:-<missing>})" >&2
            echo "  fix: update pyproject.toml [build-system].requires AND [tool.batchalign.pinned_tools].maturin" >&2
            fail=1
        fi
    fi

    # python: major.minor only (patch level is CI-controlled).
    if ! _guard_skip python; then
        local live_python
        live_python="$( (cd "$BUILD_WORKSPACE_DIRECTORY/python" \
            && "$uv" run --quiet python -c 'import sys; print(f"{sys.version_info.major}.{sys.version_info.minor}")' 2>/dev/null) || true)"
        if [[ "$live_python" != "$pin_python" ]]; then
            echo "hermeticity_guard: python version mismatch (pinned=$pin_python, live=${live_python:-<missing>})" >&2
            echo "  fix: update MODULE.bazel python.toolchain(python_version=...) AND .bazelrc python_version setting" >&2
            fail=1
        fi
    fi

    # rustc: required for the cargo invocation maturin makes.
    if ! _guard_skip rustc; then
        local live_rust
        live_rust="$(rustc --version 2>/dev/null | awk '{print $2}' || true)"
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
