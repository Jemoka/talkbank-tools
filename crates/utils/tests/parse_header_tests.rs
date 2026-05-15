//! Tests for TreeSitterParser::parse_header()
//!
//! TDD approach: Write failing tests first, then implement functionality.

#[path = "parse_header_tests/basic.rs"]
mod basic;
#[path = "parse_header_tests/helpers.rs"]
mod helpers;
#[path = "test_utils/mod.rs"]
mod test_utils;
#[path = "parse_header_tests/typed.rs"]
mod typed;
