set shell := ["bash", "-c"]
set positional-arguments := true

# Build static HTML.
build:
    bazel run //book:html

serve:
    bazel run //book:serve

linkcheck:
    bazel run //book:linkcheck
