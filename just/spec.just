set shell := ["bash", "-c"]
set positional-arguments := true

build:
    bazel build //crates/spec/...

test:
    bazel test //crates/spec/...

validate:
    bazel run //crates/spec/talkbank-spec-testrun:validate_error_specs

coverage:
    bazel run //crates/spec/talkbank-spec-testgen:coverage

gen-tree-sitter-tests:
    bazel run //crates/spec/talkbank-spec-testgen:gen_tree_sitter_tests

gen-rust-tests:
    bazel run //crates/spec/talkbank-spec-testgen:gen_rust_tests

gen-validation-tests:
    bazel run //crates/spec/talkbank-spec-testgen:gen_validation_tests

gen-error-docs:
    bazel run //crates/spec/talkbank-spec-testgen:gen_error_docs
