//! Shared testing utilities for corpus validation and roundtrip testing.
//!
//! Provides common functionality for test binaries:
//! - File discovery (find .cha files in corpus hierarchies)
//! - Corpus discovery (find directories with 0metadata.cdc)
//! - Error analysis and comparison utilities

mod error_utils;
mod file_discovery;

pub use error_utils::{error_key, errors_equal, summarize_errors};
pub use file_discovery::{count_cha_files, discover_corpora, find_cha_files};
