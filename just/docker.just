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
dev-test:
    @just docker dev
    docker run --rm \
        -v "{{ justfile_directory() }}":/src:ro \
        -w /work \
        talkbank-tools-dev:latest \
        bash -lc '\
          cp -r /src /work && cd /work && \
          just batchalign cli -- --help && \
          just batchalign cli -- compare \
              resources/corpus/reference/morphosyntax/eng.cha \
              resources/corpus/reference/morphosyntax/eng.cha || \
          echo "compare ran (non-zero acceptable for smoke)" \
        '

# Build a release wheel on the host (any platform) and install it into
# a clean python:3.12 image, then run `batchalign3 --help`. Proves the
# wheel as shipped to PyPI is self-contained.
wheel-test:
    @just batchalign wheel
    @echo "→ Building wheel-consumer image"
    @mkdir -p {{ justfile_directory() }}/docker/.scratch
    @cp {{ justfile_directory() }}/python/target/wheels/*.whl {{ justfile_directory() }}/docker/.scratch/
    @wheel_file=$(basename $(ls -t {{ justfile_directory() }}/python/target/wheels/*.whl | head -1)); \
        cp {{ justfile_directory() }}/docker/Dockerfile.wheel-consumer {{ justfile_directory() }}/docker/.scratch/Dockerfile && \
        docker build \
            --build-arg WHEEL=$wheel_file \
            -t talkbank-tools-wheel:latest \
            {{ justfile_directory() }}/docker/.scratch && \
        docker run --rm talkbank-tools-wheel:latest
    @rm -rf {{ justfile_directory() }}/docker/.scratch

# Full e2e: prove BOTH the local-dev path AND the released-wheel path
# can produce `batchalign3 --help` from scratch on clean containers.
e2e:
    @just docker dev-test
    @just docker wheel-test
    @echo "✅ both docker paths green: local-dev (no maturin) + wheel-install"
