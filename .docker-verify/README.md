# Clean-container verification

The Dockerfiles in this directory reproduce a "blank container with
only bazelisk + just installed" environment, matching what
[CONTRIBUTING.md § Host prerequisites](../CONTRIBUTING.md#host-prerequisites)
documents. Use them to confirm a host-tool change hasn't introduced a
new prerequisite leak.

```bash
# Build the base-prereqs image.
docker build -f .docker-verify/base.Dockerfile -t tb-verify:base .docker-verify

# Snapshot the current HEAD into a temp dir, then COPY it into the
# image — NOT a -v mount. colima's sshfs strips executable bits on
# Linux files, which breaks cargo's build-script-build during the
# maturin escape. ext4-backed COPY preserves them.
mkdir -p /tmp/tb-verify/workspace
(git archive --format=tar HEAD) | tar -x -C /tmp/tb-verify/workspace

cat > /tmp/tb-verify/Dockerfile.sidecar <<'EOF'
FROM tb-verify:base
COPY workspace /workspace
WORKDIR /workspace
CMD just batchalign sidecar
EOF
docker build -f /tmp/tb-verify/Dockerfile.sidecar -t tb-verify:sidecar /tmp/tb-verify

# Allocate 10G memory headroom — Bazel + cargo + maturin under load
# can spike past 6G during the cold path.
docker run --rm --memory 10g --memory-swap 10g tb-verify:sidecar
```

Same pattern works for cli (`CMD just batchalign cli -- --help`) and
GUI (`docker build -f .docker-verify/gui.Dockerfile -t tb-verify:gui
.docker-verify` first, then COPY workspace + `CMD just
batchalign::gui::build`).

Expected cold build: ~10 min for sidecar (everything Bazel-managed
gets fetched on first run). Incremental rebuilds inside a long-lived
container are sub-second.

The GUI image (`gui.Dockerfile`) adds the documented GUI-only Linux
prereqs on top of the base image — verify GUI builds with `just
batchalign::gui::build` inside that image.

These Dockerfiles are NOT used by CI; they're local sanity checks for
contributors changing the host-tool surface.
