//! Test module for error corpus in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

#[path = "error_corpus/corpus_tests.rs"]
mod corpus_tests;
#[path = "error_corpus/coverage.rs"]
mod coverage;
/// Error Corpus Test Suite
///
/// This test suite validates that our error corpus files correctly trigger
/// their expected error codes. Each .cha file in tests/error_corpus/ contains
/// @Comment headers documenting the expected error.
///
/// The test harness:
/// 1. Discovers all .cha files in error_corpus directories
/// 2. Extracts expected error code from @Comment headers
/// 3. Parses and validates each file
/// 4. Verifies expected error is present
/// 5. Uses insta snapshots for regression testing
#[path = "error_corpus/helpers.rs"]
mod helpers;
#[path = "test_utils/mod.rs"]
mod test_utils;
