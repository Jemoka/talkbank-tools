//! Test module for io in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use std::fs;

/// Enum variants for IoError.
#[derive(Debug, thiserror::Error)]
pub enum IoError {
    #[error("Failed to write file {path}: {source}")]
    Write {
        path: String,
        source: std::io::Error,
    },
}

/// Build generated and canonical schema paths for one schema stem.
pub fn schema_paths_for(schema_stem: &str) -> (String, String) {
    (
        format!("schema/{schema_stem}.generated.json"),
        format!("schema/{schema_stem}.json"),
    )
}

/// Updates schema files.
pub fn write_schema_files(
    generated_path: &str,
    canonical_path: &str,
    schema_json: &str,
) -> Result<(), IoError> {
    fs::write(generated_path, schema_json).map_err(|source| IoError::Write {
        path: generated_path.to_string(),
        source,
    })?;
    fs::write(canonical_path, schema_json).map_err(|source| IoError::Write {
        path: canonical_path.to_string(),
        source,
    })?;
    Ok(())
}

/// Prints summary.
pub fn print_summary(generated_path: &str, canonical_path: &str, length: usize) {
    println!("\n========== GENERATED JSON SCHEMA ==========");
    println!("Saved to: {}", generated_path);
    println!("Canonical: {}", canonical_path);
    println!("Length: {} bytes", length);
    println!("==========================================\n");
}
