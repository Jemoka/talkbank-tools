//! Tests for TreeSitterParser::parse_chat_file()
//!
//! TDD approach: Write failing tests first, then implement functionality.

#[path = "parse_chat_file_tests/basic.rs"]
mod basic;
#[path = "parse_chat_file_tests/dependent_tiers.rs"]
mod dependent_tiers;
#[path = "parse_chat_file_tests/helpers.rs"]
mod helpers;
#[path = "parse_chat_file_tests/realistic.rs"]
mod realistic;
#[path = "parse_chat_file_tests/roundtrip.rs"]
mod roundtrip;
#[path = "test_utils/mod.rs"]
mod test_utils;
