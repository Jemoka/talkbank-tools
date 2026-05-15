//! Test module for generate in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

mod io;
mod metadata;
mod transform;

use schemars::schema_for;
use talkbank_model::ChatFile;

/// Enum variants for TestError.
#[derive(Debug, thiserror::Error)]
enum TestError {
    #[error("Metadata error: {source}")]
    Metadata { source: metadata::MetadataError },
    #[error("IO error: {source}")]
    Io { source: io::IoError },
}

/// Generates chat file schema.
#[test]
fn generate_chat_file_schema() -> Result<(), TestError> {
    let schema = schema_for!(ChatFile);
    let mut schema_value =
        metadata::schema_to_value(schema).map_err(|source| TestError::Metadata { source })?;

    // Fix schemars bug: internally-tagged enums with $ref generate invalid JSON Schema.
    // See transform.rs for details.
    transform::fix_ref_properties_combination(&mut schema_value);

    metadata::add_schema_metadata(
        &mut schema_value,
        "https://talkbank.org/schemas/v0.1/chat-file.json",
        "JSON Schema for TalkBank CHAT format transcript files. \
         This schema defines the structure of CHAT files when serialized to JSON.",
        "modify src/model/*.rs types and run `cargo test --test generate_schema`",
    );

    let schema_json =
        metadata::to_pretty_json(&schema_value).map_err(|source| TestError::Metadata { source })?;
    let (generated_path, canonical_path) = io::schema_paths_for("chat-file.schema");
    io::write_schema_files(&generated_path, &canonical_path, &schema_json)
        .map_err(|source| TestError::Io { source })?;
    io::print_summary(&generated_path, &canonical_path, schema_json.len());

    Ok(())
}
