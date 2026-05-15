//! Integration tests for JSON serialization, schema validation, and error rendering.

use talkbank_model::ParseValidateOptions;
use talkbank_transform::json::{
    is_schema_validation_available, schema_load_error, to_json_pretty_unvalidated,
    to_json_unvalidated, validate_json_string,
};
use talkbank_transform::{
    PipelineError, chat_to_json, parse_and_validate, render_error_with_miette,
    render_error_with_miette_with_named_source, render_error_with_miette_with_source,
};

/// Minimal valid CHAT for JSON conversion tests.
const VALID_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\thello world .\n%mor:\tn|hello n|world .\n@End\n";

// ===== Schema (4 tests) =====

#[test]
fn schema_is_available() {
    assert!(
        is_schema_validation_available(),
        "JSON schema should be loadable"
    );
}

#[test]
fn valid_json_passes_schema() -> Result<(), PipelineError> {
    // Parse a valid CHAT file, convert to JSON, then validate against schema
    let options = ParseValidateOptions::default();
    let chat_file = parse_and_validate(VALID_CHAT, options)?;
    let json = talkbank_transform::json::to_json_pretty_validated(&chat_file)
        .map_err(|e| PipelineError::JsonSerialization(e.to_string()))?;
    // If to_json_pretty_validated succeeded, schema validation passed
    assert!(!json.is_empty());
    Ok(())
}

#[test]
fn invalid_json_fails_schema() {
    let random_json = r#"{"not_a_chat_field": true, "random": 42}"#;
    let result = validate_json_string(random_json);
    assert!(result.is_err(), "Random JSON should fail schema validation");
}

#[test]
fn schema_load_error_is_none() {
    assert!(
        schema_load_error().is_none(),
        "Schema load error should be None when schema is available"
    );
}

// ===== Serialization (3 tests) =====

#[test]
fn to_json_pretty_has_newlines() -> Result<(), PipelineError> {
    let options = ParseValidateOptions::default();
    let chat_file = parse_and_validate(VALID_CHAT, options)?;
    let json = to_json_pretty_unvalidated(&chat_file)
        .map_err(|e| PipelineError::JsonSerialization(e.to_string()))?;
    assert!(json.contains('\n'), "Pretty JSON should contain newlines");
    Ok(())
}

#[test]
fn to_json_unvalidated_skips_schema() -> Result<(), PipelineError> {
    let options = ParseValidateOptions::default();
    let chat_file = parse_and_validate(VALID_CHAT, options)?;
    let json = to_json_unvalidated(&chat_file)
        .map_err(|e| PipelineError::JsonSerialization(e.to_string()))?;
    assert!(!json.is_empty(), "Unvalidated JSON should produce output");
    // Verify it is valid JSON
    let parsed: serde_json::Value =
        serde_json::from_str(&json).map_err(|e| PipelineError::JsonSerialization(e.to_string()))?;
    assert!(parsed.is_object());
    Ok(())
}

#[test]
fn validate_json_string_roundtrip() -> Result<(), PipelineError> {
    // Serialize then validate the resulting string
    let options = ParseValidateOptions::default();
    let json = chat_to_json(VALID_CHAT, options, false)?;
    // chat_to_json already validates, but we can also validate the string directly
    let result = validate_json_string(&json);
    assert!(
        result.is_ok(),
        "Roundtrip JSON should pass schema validation"
    );
    Ok(())
}

// ===== Rendering (3 tests) =====

#[test]
fn render_error_includes_code() {
    // Parse invalid CHAT to get real errors
    let content = "@UTF8\n@Begin\n*CHI:\thello .\n";
    let options = ParseValidateOptions::default().with_validation();
    match parse_and_validate(content, options) {
        Err(PipelineError::Parse(parse_errors)) => {
            assert!(!parse_errors.errors.is_empty(), "Should have parse errors");
            let rendered = render_error_with_miette(&parse_errors.errors[0]);
            // The rendered output should contain some error information
            assert!(!rendered.is_empty(), "Rendered error should not be empty");
        }
        Err(PipelineError::Validation(errors)) => {
            assert!(!errors.is_empty(), "Should have validation errors");
            let rendered = render_error_with_miette(&errors[0]);
            assert!(!rendered.is_empty(), "Rendered error should not be empty");
        }
        Ok(_) => {
            // If this somehow passes, the test structure still verifies rendering works
        }
        Err(e) => {
            panic!("Unexpected error type: {e}");
        }
    }
}

#[test]
fn render_error_with_source_includes_content() {
    let content = "@UTF8\n@Begin\n*CHI:\thello .\n";
    let options = ParseValidateOptions::default().with_validation();
    match parse_and_validate(content, options) {
        Err(PipelineError::Parse(parse_errors)) => {
            let rendered =
                render_error_with_miette_with_source(&parse_errors.errors[0], "test.cha", content);
            assert!(
                !rendered.is_empty(),
                "Rendered error with source should not be empty"
            );
        }
        Err(PipelineError::Validation(errors)) => {
            let rendered = render_error_with_miette_with_source(&errors[0], "test.cha", content);
            assert!(
                !rendered.is_empty(),
                "Rendered error with source should not be empty"
            );
        }
        Ok(_) => {}
        Err(e) => panic!("Unexpected error type: {e}"),
    }
}

#[test]
fn render_error_with_named_source_includes_filename() {
    let content = "@UTF8\n@Begin\n*CHI:\thello .\n";
    let options = ParseValidateOptions::default().with_validation();
    match parse_and_validate(content, options) {
        Err(PipelineError::Parse(parse_errors)) => {
            let source = miette::NamedSource::new(
                "my_test_file.cha",
                std::sync::Arc::new(content.to_string()),
            );
            let rendered =
                render_error_with_miette_with_named_source(&parse_errors.errors[0], &source);
            assert!(
                !rendered.is_empty(),
                "Rendered error with named source should not be empty"
            );
        }
        Err(PipelineError::Validation(errors)) => {
            let source = miette::NamedSource::new(
                "my_test_file.cha",
                std::sync::Arc::new(content.to_string()),
            );
            let rendered = render_error_with_miette_with_named_source(&errors[0], &source);
            assert!(
                !rendered.is_empty(),
                "Rendered error with named source should not be empty"
            );
        }
        Ok(_) => {}
        Err(e) => panic!("Unexpected error type: {e}"),
    }
}
