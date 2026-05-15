//! Mutation-based validation tests
//!
//! Each test takes a known-good CHAT structure, applies a specific mutation,
//! and verifies the expected error code is produced.
//!
//! ## Test Naming Convention
//! `test_<error_code>_<description>` e.g., `test_e501_missing_begin_header`
//!
//! ## Mutation Categories
//! - E5xx: Header mutations
//! - E3xx: Main tier mutations
//! - E2xx: Word-level mutations
//! - E6xx: Alignment mutations

#[path = "mutation_tests/alignment.rs"]
mod alignment;
#[path = "mutation_tests/headers.rs"]
mod headers;
#[path = "mutation_tests/helpers.rs"]
mod helpers;
#[path = "mutation_tests/main_tier.rs"]
mod main_tier;
#[path = "mutation_tests/word.rs"]
mod word;
