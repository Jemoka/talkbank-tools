//! Test module for helpers in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

// Test utilities for component roundtrip tests - some may be used in future tests
#![allow(dead_code)]

use serde_json::Value;
use std::path::PathBuf;
use thiserror::Error;

/// Enum variants for ComponentHelperError.
#[derive(Debug, Error)]
pub enum ComponentHelperError {
    #[error("Failed to read schema file: {path}")]
    ReadFile {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("Failed to parse schema JSON")]
    ParseJson { source: serde_json::Error },
    #[error("Failed to compile schema for component: {component}")]
    CompileValidator {
        component: String,
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

/// Get the full CHAT JSON Schema with all component definitions.
pub fn get_chat_schema() -> Result<Value, ComponentHelperError> {
    let path = PathBuf::from("schemas/chat-file/chat-file.schema.json");
    let schema_json = std::fs::read_to_string(&path)
        .map_err(|source| ComponentHelperError::ReadFile { path, source })?;
    serde_json::from_str(&schema_json).map_err(|source| ComponentHelperError::ParseJson { source })
}

/// Create a validator for a specific component type from the schema $defs.
pub fn get_component_validator(
    component_name: &str,
) -> Result<jsonschema::Validator, ComponentHelperError> {
    let full_schema = get_chat_schema()?;

    let component_def = full_schema["$defs"][component_name].clone();

    let component_schema = if component_def.is_null() {
        serde_json::json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "type": "object"
        })
    } else {
        let required = match component_def.get("required") {
            Some(value) => value.clone(),
            None => serde_json::json!([]),
        };
        let additional_properties = match component_def.get("additionalProperties") {
            Some(value) => value.clone(),
            None => serde_json::json!(true),
        };
        serde_json::json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "$defs": full_schema["$defs"].clone(),
            "type": "object",
            "properties": component_def["properties"].clone(),
            "required": required,
            "additionalProperties": additional_properties
        })
    };

    jsonschema::validator_for(&component_schema).map_err(|source| {
        ComponentHelperError::CompileValidator {
            component: component_name.to_string(),
            source: Box::new(source),
        }
    })
}
