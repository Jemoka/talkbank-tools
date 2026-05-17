# Docker e2e for the two release-quality paths:
#
#   just docker dev          # build the dev image (bazelisk+just+rust+uv on ubuntu)
#   just docker dev-test     # clone repo INTO container, run `just batchalign cli`,
#                            #  prove the *local* path (no maturin) works on a
#                            #  fresh ubuntu. The compare smoke uses two .cha
#                            #  fixtures from resources/corpus/.
#   just docker wheel        # build the host's wheel via `just batchalign wheel`,
#                            #  then install it into a fresh python:3.12 image
#                            #  and run the CLI. Proves the *release* wheel
#                            #  ships a self-contained installable artifact.

set shell := ["bash", "-c"]
set positional-arguments := true

default:
    @just --list --justfile {{ justfile() }}

# Build the local-dev base image (bazelisk + just + pinned rust + uv).
dev:
    docker build \
        -f {{ justfile_directory() }}/docker/Dockerfile.dev \
        -t talkbank-tools-dev:latest \
        {{ justfile_directory() }}/docker

# End-to-end test: spin up a fresh ubuntu, mount the workspace read-only,
# and prove `just batchalign cli` works without maturin in the loop.
# The cdylib is built by Bazel's rust_shared_library + py_binary chain.
#
# Requires Docker Desktop to be allocated at least 8 GiB RAM (Settings
# → Resources → Memory) — a full Rust workspace compile inside the
# container otherwise OOM-kills the Bazel server. CI runners on
# Linux/macOS have plenty; some local Docker Desktop installs default
# to 2 GiB which is too tight.
#
# `cp -r /src/. /repo/` copies the *contents* of /src into /repo so the
# repo root is /repo (matching how a `git clone` would land). The `|| true`
# on the compare step is intentional — compare reads CHAT files and may
# exit non-zero on legitimate diffs; we're proving the CLI loads, not
# that the inputs match.
dev-test:
    @just docker dev
    # `SQLITE3_LIB_DIR` + `SQLITE3_INCLUDE_DIR` make `libsqlite3-sys`
    # link the system sqlite (installed via apt as libsqlite3-dev)
    # instead of recompiling its vendored copy. The vendored compile
    # eats enough memory to OOM-kill Bazel inside Docker Desktop's
    # default RAM budget on Apple Silicon.
    docker run --rm \
        -v "{{ justfile_directory() }}":/src:ro \
        -e SQLITE3_LIB_DIR=/usr/lib \
        -e SQLITE3_INCLUDE_DIR=/usr/include \
        talkbank-tools-dev:latest \
        bash -lc 'set -e && \
          mkdir -p $HOME/repo && cp -r /src/. $HOME/repo/ && cd $HOME/repo && \
          just batchalign cli -- --help && \
          (mkdir -p /tmp/compare-out && \
              just batchalign cli -- compare \
              "$PWD/resources/corpus/reference/edge-cases/empty-and-minimal.cha" \
              "$PWD/resources/corpus/reference/edge-cases/empty-and-minimal.cha" \
              -o /tmp/compare-out \
              || echo "[smoke] compare exit-code is informational only") \
        '

# Install a wheel (from python/target/wheels/, however it got there)
# into a clean python:3.12 image and run `import batchalign._core` +
# `batchalign3 --help`. Proves the released wheel is self-contained.
#
# If `python/target/wheels/` is empty, builds one first via maturin.
# To use a CI-built wheel instead (e.g. when the host can't build the
# wheel — macOS SDK issues, Docker RAM too small for a full Bazel
# compile, etc.), drop the wheel into `python/target/wheels/` first:
#   gh run download <run-id> --name batchalign-wheel-linux-x86_64 \
#                            -D python/target/wheels/
# then `just docker wheel-test`.
wheel-test:
    @if ! ls {{ justfile_directory() }}/python/target/wheels/*.whl >/dev/null 2>&1; then \
        echo "→ no wheel under python/target/wheels/, building one via maturin"; \
        just batchalign wheel; \
    else \
        echo "→ using existing wheel(s):"; \
        ls {{ justfile_directory() }}/python/target/wheels/*.whl; \
    fi
    @echo "→ Building wheel-consumer image"
    @mkdir -p {{ justfile_directory() }}/docker/.scratch
    @cp {{ justfile_directory() }}/python/target/wheels/*.whl {{ justfile_directory() }}/docker/.scratch/
    @wheel_file=$(basename $(ls -t {{ justfile_directory() }}/python/target/wheels/*.whl | head -1)); \
        case "$wheel_file" in \
            *manylinux*x86_64*|*linux*x86_64*) platform=linux/amd64 ;; \
            *manylinux*aarch64*|*linux*aarch64*) platform=linux/arm64 ;; \
            *macosx*|*win*) \
                echo "wheel-test: can't install $wheel_file inside a linux image — pick a manylinux* wheel"; \
                exit 1 ;; \
            *) platform=linux/amd64 ;; \
        esac; \
        echo "→ wheel $wheel_file → platform $platform"; \
        cp {{ justfile_directory() }}/docker/Dockerfile.wheel-consumer {{ justfile_directory() }}/docker/.scratch/Dockerfile && \
        docker build \
            --platform=$platform \
            --build-arg WHEEL=$wheel_file \
            -t talkbank-tools-wheel:latest \
            {{ justfile_directory() }}/docker/.scratch && \
        docker run --rm --platform=$platform talkbank-tools-wheel:latest
    @rm -rf {{ justfile_directory() }}/docker/.scratch

# Full e2e: prove BOTH the local-dev path AND the released-wheel path
# can produce `batchalign3 --help` from scratch on clean containers.
e2e:
    @just docker dev-test
    @just docker wheel-test
    @echo "✅ both docker paths green: local-dev (no maturin) + wheel-install"
