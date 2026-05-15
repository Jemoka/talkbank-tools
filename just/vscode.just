set shell := ["bash", "-c"]
set positional-arguments := true

# Compile TypeScript via the bazel-wrapped npm script.
build:
    bazel run //apps/vscode-extension:build

# Produce .vsix.
package:
    bazel run //apps/vscode-extension:package

# Run vscode extension tests.
test:
    bazel run //apps/vscode-extension:test
