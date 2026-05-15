//! Test module for metadata in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use serde_json::Value;

/// Enum variants for MetadataError.
#[derive(Debug, thiserror::Error)]
pub enum MetadataError {
    #[error("Failed to convert schema to JSON value: {source}")]
    ToValue { source: serde_json::Error },
    #[error("Failed to serialize schema: {source}")]
    ToString { source: serde_json::Error },
}

/// Runs schema to value.
pub fn schema_to_value<T: serde::Serialize>(schema: T) -> Result<Value, MetadataError> {
    serde_json::to_value(&schema).map_err(|source| MetadataError::ToValue { source })
}

/// Adds schema metadata.
pub fn add_schema_metadata(
    schema_value: &mut Value,
    schema_id: &str,
    description: &str,
    update_command: &str,
) {
    if let Some(obj) = schema_value.as_object_mut() {
        obj.insert(
            "$schema".to_string(),
            Value::String("https://json-schema.org/draft/2020-12/schema".to_string()),
        );
        obj.insert(
            "$comment".to_string(),
            Value::String(
                "AUTO-GENERATED - Do not edit manually! \
                 This schema is automatically generated from Rust types using schemars. \
                 To update: "
                    .to_string()
                    + update_command,
            ),
        );
        obj.insert("$id".to_string(), Value::String(schema_id.to_string()));
        obj.insert(
            "description".to_string(),
            Value::String(description.to_string()),
        );
    }
}

/// Runs to pretty json.
pub fn to_pretty_json(schema_value: &Value) -> Result<String, MetadataError> {
    serde_json::to_string_pretty(schema_value).map_err(|source| MetadataError::ToString { source })
}
