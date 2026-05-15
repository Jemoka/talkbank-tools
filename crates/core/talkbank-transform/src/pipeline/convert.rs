//! Conversion pipeline helpers (CHAT <-> normalized CHAT/JSON outputs).
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::json::{
    to_json_pretty_unvalidated, to_json_pretty_validated, to_json_unvalidated, to_json_validated,
};
use talkbank_model::ParseValidateOptions;
use talkbank_model::WriteChat;

use super::error::PipelineError;
use super::parse::parse_and_validate;

/// Parse, validate, and serialize to JSON with schema validation.
///
/// This pipeline function:
/// 1. Parses CHAT content
/// 2. Validates CHAT structure (if requested via options)
/// 3. Serializes to JSON
/// 4. **Validates JSON against schema** (always)
///
/// Use [`chat_to_json_unvalidated`] to skip JSON schema validation.
///
/// # Arguments
///
/// * `content` - The CHAT file content
/// * `options` - Parsing and validation options
/// * `pretty` - Pretty-print JSON output
///
/// # Returns
///
/// * `Ok(String)` - JSON string (validated against schema)
/// * `Err(PipelineError)` - Parse, validation, or serialization error
///
/// # Example
///
/// ```no_run
/// use talkbank_transform::chat_to_json;
/// use talkbank_model::ParseValidateOptions;
///
/// # fn convert() -> Result<(), talkbank_transform::PipelineError> {
/// let content = "*CHI:\thello world .";
/// let options = ParseValidateOptions::default();
/// let _json = chat_to_json(content, options, true)?;
/// # Ok(())
/// # }
/// ```
pub fn chat_to_json(
    content: &str,
    options: ParseValidateOptions,
    pretty: bool,
) -> Result<String, PipelineError> {
    // Parse and validate
    let chat_file = parse_and_validate(content, options)?;

    // Serialize to JSON with schema validation
    let json = if pretty {
        to_json_pretty_validated(&chat_file)
    } else {
        to_json_validated(&chat_file)
    }
    .map_err(|e| PipelineError::JsonSerialization(e.to_string()))?;

    Ok(json)
}

/// Parse, validate, and serialize to JSON WITHOUT schema validation.
///
/// This pipeline function:
/// 1. Parses CHAT content
/// 2. Validates CHAT structure (if requested via options)
/// 3. Serializes to JSON (without schema validation)
///
/// **Use sparingly.** Prefer [`chat_to_json`] which validates JSON against schema.
/// This variant is useful for:
/// - Performance-critical paths where validation is done elsewhere
/// - Testing specific serialization behavior
///
/// # Arguments
///
/// * `content` - The CHAT file content
/// * `options` - Parsing and validation options
/// * `pretty` - Pretty-print JSON output
///
/// # Returns
///
/// * `Ok(String)` - JSON string (NOT validated against schema)
/// * `Err(PipelineError)` - Parse, validation, or serialization error
pub fn chat_to_json_unvalidated(
    content: &str,
    options: ParseValidateOptions,
    pretty: bool,
) -> Result<String, PipelineError> {
    // Parse and validate
    let chat_file = parse_and_validate(content, options)?;

    // Serialize to JSON WITHOUT schema validation
    let json = if pretty {
        to_json_pretty_unvalidated(&chat_file)
    } else {
        to_json_unvalidated(&chat_file)
    }
    .map_err(|e| PipelineError::JsonSerialization(e.to_string()))?;

    Ok(json)
}

/// Parse and rewrite CHAT into canonical serialized form.
///
/// This pipeline function:
/// 1. Parses CHAT content
/// 2. Validates (if requested)
/// 3. Serializes back to canonical CHAT format
///
/// # Arguments
///
/// * `content` - The CHAT file content
/// * `options` - Parsing and validation options
///
/// # Returns
///
/// * `Ok(String)` - Canonical CHAT string
/// * `Err(PipelineError)` - Parse or validation error
///
/// # Example
///
/// ```no_run
/// use talkbank_transform::normalize_chat;
/// use talkbank_model::ParseValidateOptions;
///
/// # fn normalize() -> Result<(), talkbank_transform::PipelineError> {
/// let content = "*CHI:\thello world .";
/// let options = ParseValidateOptions::default().with_validation();
/// let _normalized = normalize_chat(content, options)?;
/// # Ok(())
/// # }
/// ```
pub fn normalize_chat(
    content: &str,
    options: ParseValidateOptions,
) -> Result<String, PipelineError> {
    // Parse and validate
    let chat_file = parse_and_validate(content, options)?;

    // Serialize to CHAT format
    Ok(chat_file.to_chat_string())
}

#[cfg(test)]
mod tests {
    use super::{chat_to_json, normalize_chat};
    use crate::PipelineError;
    use talkbank_model::ParseValidateOptions;

    #[test]
    fn test_chat_to_json_pretty() -> Result<(), PipelineError> {
        let content = "@UTF8\n@Begin\n@End\n";
        let options = ParseValidateOptions::default();
        let json = chat_to_json(content, options, true)?;
        assert!(json.contains("{\n")); // Pretty-printed
        Ok(())
    }

    #[test]
    fn test_chat_to_json_compact() -> Result<(), PipelineError> {
        let content = "@UTF8\n@Begin\n@End\n";
        let options = ParseValidateOptions::default();
        let json = chat_to_json(content, options, false)?;
        assert!(!json.contains("  ")); // Not pretty-printed
        Ok(())
    }

    #[test]
    fn test_normalize_chat() -> Result<(), PipelineError> {
        let content = "@UTF8\n@Begin\n@End\n";
        let options = ParseValidateOptions::default();
        let _ = normalize_chat(content, options)?;
        Ok(())
    }
}
