//! Configuration for the alignment validation test.
//!
//! Settings are passed as command-line arguments parsed by `clap`
//! in `tests/alignment_validation.rs`.
//!
//! ## Usage (Command Line)
//!
//! ```bash
//! cargo test --test alignment_validation -- --corpus-dir <path>
//! ```

use std::path::PathBuf;

/// Get the default test corpus directory.
/// Returns the reference corpus path, which is always available.
pub fn corpus_dir() -> PathBuf {
    PathBuf::from("resources/corpus/reference")
}
