//! Error corpus generation infrastructure.
//!
//! Programmatically generates test files for all error codes to ensure 100% coverage.
//! Uses [`ChatFileBuilder`](crate::ChatFileBuilder) to create valid CHAT files with
//! specific errors for validation testing.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

pub mod alignment_errors;
pub mod header_errors;
pub mod internal_errors;
pub mod parser_errors;
pub mod tier_errors;
pub mod warnings;
pub mod word_errors;

use std::fs;
use std::path::Path;

pub use alignment_errors::generate_e7xx_alignment_errors;
pub use header_errors::generate_e5xx_header_errors;
pub use internal_errors::generate_e0_e1xx_internal_errors;
pub use parser_errors::generate_e3xx_parser_errors;
pub use tier_errors::{generate_e4xx_dependent_tier_errors, generate_e6xx_tier_errors};
pub use warnings::generate_wxxx_warnings;
pub use word_errors::generate_e2xx_word_errors;

/// Convenience type alias used by all generator functions.
pub type GenResult = Result<usize, Box<dyn std::error::Error>>;

/// Updates file.
pub fn write_file(path: &Path, content: String) -> Result<(), Box<dyn std::error::Error>> {
    fs::write(path, content)?;
    let fallback = path.to_string_lossy();
    let name = match path.file_name().and_then(|name| name.to_str()) {
        Some(name) => name,
        None => fallback.as_ref(),
    };
    println!("  Generated: {}", name);
    Ok(())
}
