# Workspace-wide runner. Replaces the former `//:<project>/<role>`
# aliases in the root BUILD.bazel. Each scope lives in its own module
# (`just <scope> <recipe>`).
#
# Discovery: `just --list` or `just <scope> --list`.

set shell := ["bash", "-c"]
set positional-arguments := true

mod chatter    "just/chatter.just"
mod batchalign "just/batchalign.just"
mod clan       "just/clan.just"
mod spec       "just/spec.just"
mod vscode     "just/vscode.just"
mod docs       "just/docs.just"
mod tooling    "just/tooling.just"

default:
    @just --list

# Build every Bazel target in the workspace.
build:
    bazel build //...

# Run every Bazel test target in the workspace.
test:
    bazel test //...
