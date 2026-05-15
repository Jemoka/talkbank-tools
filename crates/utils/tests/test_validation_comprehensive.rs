//! Comprehensive validation tests for happy and sad paths
//!
//! This test suite fills gaps in validation coverage, testing both:
//! - Happy path: Valid CHAT files that should pass validation
//! - Sad path: Invalid CHAT files that should trigger specific errors
//!
//! Note: Some tests are commented out due to tree-sitter grammar limitations.
//! See docs/VALIDATION_TEST_GAPS.md for details on what's not yet supported.

#[path = "test_validation_comprehensive/happy.rs"]
mod happy;
#[path = "test_validation_comprehensive/helpers.rs"]
mod helpers;
#[path = "test_validation_comprehensive/integration.rs"]
mod integration;
#[path = "test_validation_comprehensive/sad.rs"]
mod sad;
#[path = "test_utils/mod.rs"]
mod test_utils;
