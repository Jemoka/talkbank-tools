# Verification environment: base toolchain prereqs only (no GUI deps).
# Per CONTRIBUTING.md "Host prerequisites" + Linux section.
FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update -y \
 && apt-get install -y --no-install-recommends \
        ca-certificates curl git \
        build-essential pkg-config \
        libsqlite3-dev \
 && rm -rf /var/lib/apt/lists/*

RUN curl -fsSL -o /usr/local/bin/bazelisk \
        https://github.com/bazelbuild/bazelisk/releases/download/v1.22.0/bazelisk-linux-arm64 \
 && chmod +x /usr/local/bin/bazelisk \
 && ln -s /usr/local/bin/bazelisk /usr/local/bin/bazel

RUN curl -fsSL https://just.systems/install.sh | bash -s -- --to /usr/local/bin

WORKDIR /workspace
