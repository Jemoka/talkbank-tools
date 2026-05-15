//! Generate JSON Schema for the `talkbank/analyze` editor/server contract.
//!
//! Run with: `cargo test --test generate_analyze_command_schema -- --nocapture`

#[path = "generate_schema/io.rs"]
mod io;
#[path = "generate_schema/metadata.rs"]
mod metadata;
#[path = "generate_schema/transform.rs"]
mod transform;

use schemars::schema_for;
use chatter_lsp::backend::contracts::AnalyzeCommandPayload;

/// Errors surfaced while generating the analyze command schema.
#[derive(Debug, thiserror::Error)]
enum TestError {
    #[error("Metadata error: {source}")]
    Metadata { source: metadata::MetadataError },
    #[error("IO error: {source}")]
    Io { source: io::IoError },
}

/// Generates the `talkbank/analyze` contract schema from Rust types.
#[test]
fn generate_analyze_command_schema() -> Result<(), TestError> {
    let schema = schema_for!(AnalyzeCommandPayload);
    let mut schema_value =
        metadata::schema_to_value(schema).map_err(|source| TestError::Metadata { source })?;

    transform::fix_ref_properties_combination(&mut schema_value);

    metadata::add_schema_metadata(
        &mut schema_value,
        "https://talkbank.org/schemas/v0.1/analyze-command.json",
        "JSON Schema for the `talkbank/analyze` execute-command payload shared by the TalkBank VS Code extension and language server.",
        "modify crates/chatter-lsp/src/backend/contracts.rs and run `cargo test --test generate_analyze_command_schema`",
    );

    let schema_json =
        metadata::to_pretty_json(&schema_value).map_err(|source| TestError::Metadata { source })?;
    let (generated_path, canonical_path) = io::schema_paths_for("analyze-command.schema");
    io::write_schema_files(&generated_path, &canonical_path, &schema_json)
        .map_err(|source| TestError::Io { source })?;
    io::print_summary(&generated_path, &canonical_path, schema_json.len());

    Ok(())
}
