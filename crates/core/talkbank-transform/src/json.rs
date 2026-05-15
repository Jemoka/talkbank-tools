#![allow(missing_docs)]
//! JSON serialization with schema validation.
//!
//! This module provides JSON serialization that validates output against the JSON Schema.
//! **All JSON generation in this project should use these functions.**
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//!
//! # Default Behavior
//!
//! By default, all serialization validates against the schema. This catches:
//! - Schema drift (model changes without schema regeneration)
//! - Serde/schemars attribute mismatches
//! - Invalid JSON structure
//!
//! # Examples
//!
//! ```
//! use talkbank_transform::json::to_json_unvalidated;
//!
//! // Serialize any serde type to JSON (without schema validation)
//! let value = serde_json::json!({"headers": [], "lines": []});
//! let json = to_json_unvalidated(&value).unwrap();
//! assert!(json.contains("headers"));
//! ```
//!
//! # Schema Location
//!
//! The schema is embedded at compile time from `schemas/chat-file/chat-file.schema.json`.

use serde::Serialize;
use std::sync::LazyLock;
use thiserror::Error;

/// The CHAT JSON Schema, embedded at compile time.
pub const SCHEMA_JSON: &str = include_str!("../../../../schemas/chat-file/chat-file.schema.json");

/// Errors that can occur during JSON serialization or validation.
#[derive(Debug, Error)]
pub enum JsonError {
    /// JSON serialization failed
    #[error("JSON serialization failed: {0}")]
    SerializationError(#[from] serde_json::Error),

    /// Schema validation failed
    #[error("JSON schema validation failed: {message}")]
    SchemaValidationError {
        /// Detailed error message describing schema validation failures.
        message: String,
    },

    /// Schema could not be loaded
    #[error("Failed to load JSON schema: {0}")]
    SchemaLoadError(String),
}

/// Result type for JSON operations.
pub type JsonResult<T> = Result<T, JsonError>;

/// Compiled JSON schema validator (loaded once, reused for all validations).
static SCHEMA_VALIDATOR: LazyLock<Result<jsonschema::Validator, String>> =
    LazyLock::new(load_schema_validator);

/// Load and compile the JSON schema validator from the embedded schema.
fn load_schema_validator() -> Result<jsonschema::Validator, String> {
    let schema_value: serde_json::Value = serde_json::from_str(SCHEMA_JSON)
        .map_err(|e| format!("Failed to parse schema JSON: {e}"))?;
    jsonschema::validator_for(&schema_value).map_err(|e| format!("Failed to compile schema: {e}"))
}

/// Serialize to JSON with schema validation (recommended).
///
/// This is the preferred method for JSON serialization. It validates the output
/// against the JSON schema to catch schema drift and serialization bugs.
///
/// # Errors
///
/// Returns an error if:
/// - JSON serialization fails
/// - The output doesn't validate against the schema
/// - The schema couldn't be loaded
pub fn to_json_validated<T: Serialize>(value: &T) -> JsonResult<String> {
    let json_string = serde_json::to_string(value)?;
    validate_json_string(&json_string)?;
    Ok(json_string)
}

/// Serialize to pretty-printed JSON with schema validation (recommended).
///
/// Same as `to_json_validated` but with indentation for readability.
pub fn to_json_pretty_validated<T: Serialize>(value: &T) -> JsonResult<String> {
    let json_string = serde_json::to_string_pretty(value)?;
    validate_json_string(&json_string)?;
    Ok(json_string)
}

/// Serialize to JSON WITHOUT schema validation.
///
/// **Use sparingly.** This bypasses schema validation and should only be used when:
/// - Performance is critical and you've already validated elsewhere
/// - You're serializing intermediate data that doesn't need schema compliance
///
/// For `ChatFile` serialization, prefer `to_json_validated`.
pub fn to_json_unvalidated<T: Serialize>(value: &T) -> JsonResult<String> {
    Ok(serde_json::to_string(value)?)
}

/// Serialize to pretty-printed JSON WITHOUT schema validation.
///
/// See `to_json_unvalidated` for when to use this.
pub fn to_json_pretty_unvalidated<T: Serialize>(value: &T) -> JsonResult<String> {
    Ok(serde_json::to_string_pretty(value)?)
}

/// Validate a JSON string against the schema.
///
/// This is useful when you already have a JSON string and want to validate it.
pub fn validate_json_string(json_string: &str) -> JsonResult<()> {
    let validator = SCHEMA_VALIDATOR
        .as_ref()
        .map_err(|e| JsonError::SchemaLoadError(e.clone()))?;

    let json_value: serde_json::Value = serde_json::from_str(json_string)?;

    // Use iter_errors to get all validation errors
    let error_messages: Vec<String> = validator
        .iter_errors(&json_value)
        .map(|e| format!("  - {}: {}", e.instance_path(), e))
        .collect();

    if !error_messages.is_empty() {
        return Err(JsonError::SchemaValidationError {
            message: format!(
                "JSON does not conform to schema:\n{}",
                error_messages.join("\n")
            ),
        });
    }

    Ok(())
}

/// Check if schema validation is available.
///
/// Returns `true` if the schema was loaded successfully and validation can be performed.
/// Returns `false` if the schema couldn't be loaded (validation will fail).
pub fn is_schema_validation_available() -> bool {
    SCHEMA_VALIDATOR.is_ok()
}

/// Get the schema load error, if any.
///
/// Returns `None` if schema loaded successfully, or `Some(error_message)` if it failed.
pub fn schema_load_error() -> Option<&'static str> {
    SCHEMA_VALIDATOR.as_ref().err().map(|s| s.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests schema loads.
    #[test]
    fn test_schema_loads() {
        assert!(
            is_schema_validation_available(),
            "Schema should be loadable. Error: {:?}",
            schema_load_error()
        );
    }

    /// Tests valid json passes validation.
    #[test]
    fn test_valid_json_passes_validation() {
        // Minimal valid ChatFile JSON
        let valid_json = r#"{
            "headers": [],
            "lines": []
        }"#;

        assert!(validate_json_string(valid_json).is_ok());
    }

    /// Tests invalid json fails validation.
    #[test]
    fn test_invalid_json_fails_validation() {
        // Missing required fields
        let invalid_json = r#"{
            "not_a_valid_field": true
        }"#;

        let result = validate_json_string(invalid_json);
        assert!(result.is_err());
    }
}
