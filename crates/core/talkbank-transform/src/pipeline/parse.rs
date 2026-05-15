//! Parse/validate pipeline entry points for CHAT content.
//!
//! This module provides pipeline functions that compose parsing and validation.
//! Most callers should use `parse_and_validate()` or `parse_and_validate_streaming()`.
//! For batch workflows where parser construction overhead matters, use the
//! `_with_parser` variants that accept a caller-provided `TreeSitterParser`.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use talkbank_model::ChatFile;
use talkbank_model::ParseOutcome;
use talkbank_model::ParseValidateOptions;
use talkbank_model::{ErrorCode, ErrorCollector, ErrorSink, ParseError, ParseErrors, Severity};
use talkbank_parser::TreeSitterParser;

use super::error::PipelineError;

/// Parse CHAT content and optionally validate.
///
/// This is the core pipeline function that:
/// 1. Creates a TreeSitterParser
/// 2. Parses CHAT content to ChatFile
/// 3. Optionally validates the data model
/// 4. Optionally validates tier alignment
///
/// # Arguments
///
/// * `content` - The CHAT file content as a string
/// * `options` - Parsing and validation options
///
/// # Returns
///
/// * `Ok(ChatFile)` - Successfully parsed (and validated if requested)
/// * `Err(PipelineError)` - Parse or validation errors
///
/// # Example
///
/// ```no_run
/// use talkbank_transform::{parse_and_validate, PipelineError};
/// use talkbank_model::ParseValidateOptions;
///
/// # fn parse_example() -> Result<(), PipelineError> {
/// let content = "*CHI:\thello world .";
/// let options = ParseValidateOptions::default().with_validation();
/// let chat_file = parse_and_validate(content, options)?;
/// # let _ = chat_file;
/// # Ok(())
/// # }
/// ```
pub fn parse_and_validate(
    content: &str,
    options: ParseValidateOptions,
) -> Result<ChatFile, PipelineError> {
    let parser =
        TreeSitterParser::new().map_err(|e| PipelineError::ParserCreation(format!("{e}")))?;
    parse_and_validate_with_parser(&parser, content, options)
}

/// Parse CHAT content and optionally validate using a caller-provided TreeSitterParser.
///
/// This avoids per-call parser construction, which is useful for batch workflows.
pub fn parse_and_validate_with_parser(
    parser: &TreeSitterParser,
    content: &str,
    options: ParseValidateOptions,
) -> Result<ChatFile, PipelineError> {
    let parse_errors = ErrorCollector::new();

    let chat_file_outcome = parser.parse_chat_file_fragment(content, 0, &parse_errors);

    let parse_error_vec = parse_errors.into_vec();
    let actual_errors: Vec<_> = parse_error_vec
        .iter()
        .filter(|e| e.severity == Severity::Error)
        .cloned()
        .collect();

    if !actual_errors.is_empty() {
        return Err(PipelineError::Parse(ParseErrors {
            errors: parse_error_vec,
        }));
    }

    let mut chat_file = match chat_file_outcome {
        ParseOutcome::Parsed(chat_file) => chat_file,
        ParseOutcome::Rejected => {
            return Err(PipelineError::ParserCreation(
                "Parser rejected input without reporting errors".to_string(),
            ));
        }
    };

    if options.validate || options.alignment {
        let validation_errors = ErrorCollector::new();

        // Build a model-level ValidationConfig when strict-linkers is requested.
        let model_config = if options.strict_linkers {
            Some(talkbank_model::ValidationConfig::new().with_strict_linkers())
        } else {
            None
        };

        if options.alignment {
            // validate_with_alignment does not accept a config — it calls
            // validate() internally. For strict-linkers support we fall back
            // to validate_with_config (which runs the same checks minus
            // the pre-alignment computation). The alignment pass is implicit
            // when the ChatFile already has alignments computed.
            if let Some(config) = model_config {
                chat_file.validate_with_config(config, &validation_errors, None);
            } else {
                chat_file.validate_with_alignment(&validation_errors, None);
            }
        } else if let Some(config) = model_config {
            chat_file.validate_with_config(config, &validation_errors, None);
        } else {
            chat_file.validate(&validation_errors, None);
        }

        let validation_error_vec = validation_errors.into_vec();
        let has_validation_errors = validation_error_vec
            .iter()
            .any(|e| matches!(e.severity, Severity::Error));
        if has_validation_errors {
            return Err(PipelineError::Validation(validation_error_vec));
        }
    }

    Ok(chat_file)
}

/// Parse CHAT content and optionally validate with streaming error reporting.
///
/// This is the streaming variant that accepts an ErrorSink for real-time error reporting.
/// Errors are reported immediately as they're discovered, enabling:
/// - Real-time error display in interactive environments
/// - Early cancellation (user can Ctrl+C after seeing first errors)
/// - Memory efficiency (no need to accumulate all errors)
///
/// # Arguments
///
/// * `content` - The CHAT file content as a string
/// * `options` - Parsing and validation options
/// * `errors` - ErrorSink that receives errors as they're discovered
///
/// # Returns
///
/// * `ChatFile` - Always returns a ChatFile (even if there were errors)
///
/// # Example
///
/// ```no_run
/// use talkbank_transform::parse_and_validate_streaming;
/// use talkbank_model::ParseValidateOptions;
/// use talkbank_model::ErrorCollector;
///
/// let content = "*CHI:\thello world .";
/// let options = ParseValidateOptions::default().with_validation();
/// let errors = ErrorCollector::new();
/// let chat_file = parse_and_validate_streaming(content, options, &errors);
/// // Errors are in the sink, file is always returned for recovery
/// ```
pub fn parse_and_validate_streaming(
    content: &str,
    options: ParseValidateOptions,
    errors: &impl ErrorSink,
) -> Result<ChatFile, PipelineError> {
    let parser =
        TreeSitterParser::new().map_err(|e| PipelineError::ParserCreation(format!("{e}")))?;
    parse_and_validate_streaming_with_parser(&parser, content, options, errors)
}

/// Streaming variant that reuses a caller-provided parser instance.
pub fn parse_and_validate_streaming_with_parser(
    parser: &TreeSitterParser,
    content: &str,
    options: ParseValidateOptions,
    errors: &impl ErrorSink,
) -> Result<ChatFile, PipelineError> {
    let chat_file_outcome = parser.parse_chat_file_fragment(content, 0, errors);

    let mut chat_file = match chat_file_outcome {
        ParseOutcome::Parsed(chat_file) => chat_file,
        ParseOutcome::Rejected => {
            let parse_error = ParseError::build(ErrorCode::ParseFailed)
                .message("Parser rejected input without reporting errors")
                .finish()
                .map_err(|err| PipelineError::ParserCreation(err.to_string()))?;
            errors.report(parse_error);
            ChatFile::new(vec![])
        }
    };

    if options.validate || options.alignment {
        // Build a model-level ValidationConfig when strict-linkers is requested.
        let model_config = if options.strict_linkers {
            Some(talkbank_model::ValidationConfig::new().with_strict_linkers())
        } else {
            None
        };

        if options.alignment {
            if let Some(config) = model_config {
                chat_file.validate_with_config(config, errors, None);
            } else {
                chat_file.validate_with_alignment(errors, None);
            }
        } else if let Some(config) = model_config {
            chat_file.validate_with_config(config, errors, None);
        } else {
            chat_file.validate(errors, None);
        }
    }

    Ok(chat_file)
}

#[cfg(test)]
mod tests {
    use super::{PipelineError, parse_and_validate, parse_and_validate_with_parser};
    use talkbank_model::ErrorCode;
    use talkbank_model::ParseValidateOptions;
    use talkbank_parser::TreeSitterParser;

    #[test]
    fn test_span_preserved_through_pipeline() -> Result<(), PipelineError> {
        let content = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI||||Target_Child|||\n*CHI:\thello world\n@End\n";

        let options = ParseValidateOptions::default().with_validation();

        match parse_and_validate(content, options) {
            Ok(chat_file) => {
                // Check that main tier has proper span
                let utterance = match chat_file.utterances().next() {
                    Some(utterance) => utterance,
                    None => {
                        return Err(PipelineError::ParserCreation(
                            "Missing utterance in parsed file".to_string(),
                        ));
                    }
                };
                let main_tier = &utterance.main;

                println!(
                    "Main tier span: {}..{}",
                    main_tier.span.start, main_tier.span.end
                );
                assert_ne!(main_tier.span.start, 0, "Span should not be 0..0");
                assert_ne!(main_tier.span.end, 0, "Span should not be 0..0");
            }
            Err(PipelineError::Validation(errors)) => {
                // Should have validation errors (missing terminator)
                println!("Got validation errors (expected):");
                for error in &errors {
                    println!(
                        "  Error: {} at span {}..{}",
                        error.message, error.location.span.start, error.location.span.end
                    );

                    if error.code == ErrorCode::MissingSpeaker {
                        assert_ne!(
                            error.location.span.start, 0,
                            "E304 error span should not be 0..0"
                        );
                        assert_ne!(
                            error.location.span.end, 0,
                            "E304 error span should not be 0..0"
                        );
                    }
                }
            }
            Err(e) => return Err(e),
        }

        Ok(())
    }

    #[test]
    fn test_parse_and_validate_simple() {
        let content = "@UTF8\n@Begin\n@End\n";
        let options = ParseValidateOptions::default();
        let result = parse_and_validate(content, options);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_and_validate_with_validation() -> Result<(), PipelineError> {
        // Use minimal valid CHAT file (validation may require certain headers)
        let content = "@UTF8\n@Begin\n@End\n";
        let options = ParseValidateOptions::default().with_validation();
        let result = parse_and_validate(content, options);
        // Validation may find missing required elements, so we just check it doesn't panic
        match result {
            Ok(_) => {}                             // Validation passed
            Err(PipelineError::Validation(_)) => {} // Validation failed as expected for minimal file
            Err(e) => return Err(e),
        }
        Ok(())
    }

    #[test]
    fn test_with_explicit_parser() -> Result<(), PipelineError> {
        let content = "@UTF8\n@Begin\n@End\n";
        let options = ParseValidateOptions::default();

        let parser = TreeSitterParser::new()
            .map_err(|err| PipelineError::ParserCreation(format!("{:?}", err)))?;
        let result = parse_and_validate_with_parser(&parser, content, options);

        assert!(result.is_ok());

        Ok(())
    }
}
