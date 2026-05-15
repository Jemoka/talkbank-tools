set shell := ["bash", "-c"]
set positional-arguments := true

build:
    bazel build //crates/batchalign/... //apps/batchalign/... //python/batchalign/...

test:
    bazel test //crates/batchalign/... //apps/batchalign/... //python/batchalign/...

cli *args:
    bazel run //crates/batchalign/batchalign-cli:batchalign3 -- "$@"

wheel:
    bazel run //python/batchalign:wheel

develop:
    bazel run //python/batchalign:develop

pytest *args:
    bazel run //python/batchalign:test -- "$@"

lint:
    bazel run //python/batchalign:lint

dashboard:
    bazel build //apps/batchalign/batchalign-cli-webdashboard:dist

desktop *args:
    bazel run //apps/batchalign/batchalign-gui-dashboard/src-tauri:batchalign-gui-dashboard -- "$@"
