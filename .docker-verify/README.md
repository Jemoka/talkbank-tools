# Clean-container verification

The Dockerfiles in this directory reproduce a "blank container with
only bazelisk + just installed" environment, matching what
[CONTRIBUTING.md § Host prerequisites](../CONTRIBUTING.md#host-prerequisites)
documents. Use them to confirm a host-tool change hasn't introduced a
new prerequisite leak.

```bash
# Snapshot the current HEAD into a buildable directory.
mkdir -p /tmp/tb-verify-work
(cd <workspace> && git archive --format=tar HEAD) | tar -x -C /tmp/tb-verify-work

# Build the base-prereqs image.
docker build -f .docker-verify/base.Dockerfile -t tb-verify:base .docker-verify

# Run the sidecar build inside it (workspace mounted; for sshfs hosts
# like colima, COPY-into-image instead of -v mount).
docker run --rm --memory 10g -v /tmp/tb-verify-work:/workspace tb-verify:base \
    bash -c "just batchalign sidecar"
```

Expected cold build: ~10 min for sidecar (everything Bazel-managed
gets fetched on first run). Incremental rebuilds inside a long-lived
container are sub-second.

The GUI image (`gui.Dockerfile`) adds the documented GUI-only Linux
prereqs on top of the base image — verify GUI builds with `just
batchalign::gui::build` inside that image.

These Dockerfiles are NOT used by CI; they're local sanity checks for
contributors changing the host-tool surface.
