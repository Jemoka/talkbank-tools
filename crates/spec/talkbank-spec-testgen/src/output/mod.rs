//! # Output Formatters
//!
//! Code generators that transform parsed spec types into downstream artifacts.
//!
//! Each sub-module targets a different output format:
//! - [`tree_sitter`] -- tree-sitter corpus test files (`*.txt`) written to
//!   `tree-sitter-talkbank/test/corpus/`.
//! - [`rust_test`] -- Rust `#[test]` source files for parser and validation
//!   crates.
//! - [`markdown`] -- Markdown documentation pages (error catalogs, construct
//!   references).

pub mod markdown;
pub mod rust_test;
pub mod tree_sitter;
