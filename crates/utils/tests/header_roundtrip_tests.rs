//! Roundtrip tests for typed headers
//!
//! Verifies that parsing and serialization produce identical results.

#[path = "header_roundtrip_tests/basic.rs"]
mod basic;
#[path = "header_roundtrip_tests/helpers.rs"]
mod helpers;
#[path = "test_utils/mod.rs"]
mod test_utils;
#[path = "header_roundtrip_tests/typed.rs"]
mod typed;
