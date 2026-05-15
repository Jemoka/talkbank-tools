//! Test module for generated tests in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

// Integration test for generated tests from talkbank-chat-spec
//
// Tests are generated from resources/spec/constructs/ and resources/spec/errors/ via `just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs`.
// Validation coverage (semantic errors E5xx, E6xx, E7xx) is provided by:
//   - tests/roundtrip_corpus (339 reference files, must pass 100%)
//   - tests/error_corpus (per-error fixtures exercised via chatter validate)
// The gen_validation_tests tool exists for future automation but is not
// wired into just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs because many auto-generated specs have incorrect
// layer classifications that would produce unreliable tests.

// Shared imports
use talkbank_parser::TreeSitterParser;

mod construct_tests {
    use super::*;
    include!("generated/generated_construct_tests_body.rs");
}

#[allow(unused_imports)]
mod error_tests {
    use super::*;
    include!("generated/generated_error_tests_body.rs");
}
