//! # Rust Test Generator
//!
//! Generates Rust test files from specifications

use crate::spec::construct::{ConstructExample, ConstructSpec};
use crate::spec::error::{ErrorDefinition, ErrorExample, ErrorSpec};

/// Runs wrap for chat file parse.
fn wrap_for_chat_file_parse(example: &ConstructExample, level: &str) -> String {
    let input_type = example.input_type.trim();
    let chat_prelude = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||";
    let has_chat_boundaries = example.input.contains("@Begin") && example.input.contains("@End");

    match input_type {
        // Complete documents should be parsed as-is; fragments in chat blocks are wrapped.
        "chat" | "chat-file" | "document" => {
            if has_chat_boundaries {
                example.input.clone()
            } else {
                format!("{chat_prelude}\n{}\n@End", example.input)
            }
        }
        // Header fragments must be wrapped in minimal file structure.
        "languages_header" | "participants_header" => {
            if input_type == "participants_header" {
                let speaker = extract_participant_speaker(&example.input).unwrap_or("CHI");
                format!(
                    "@UTF8\n@Begin\n@Languages:\teng\n{}\n@ID:\teng|corpus|{}|||||Target_Child|||\n@End",
                    example.input, speaker
                )
            } else {
                format!("@UTF8\n@Begin\n{}\n@End", example.input)
            }
        }
        // Main tier / utterance fragments: input is already a complete tier line.
        "main_tier" | "utterance" => {
            format!("{chat_prelude}\n{}\n@End", example.input)
        }
        // Dependent tier fragments require a main tier anchor.
        "com_dependent_tier" | "gra_dependent_tier" | "mor_dependent_tier"
        | "pho_dependent_tier" => {
            format!("{chat_prelude}\n*CHI:\tword .\n{}\n@End", example.input)
        }
        // Default handling by construct level (directory name).
        _ => match level {
            "word" => format!("{chat_prelude}\n*CHI:\t{} .\n@End", example.input),
            "main_tier" | "utterance" => format!("{chat_prelude}\n{}\n@End", example.input),
            "header" => format!("@UTF8\n@Begin\n{}\n@End", example.input),
            "tiers" => format!("{chat_prelude}\n*CHI:\tword .\n{}\n@End", example.input),
            _ => format!("{chat_prelude}\n{}\n@End", example.input),
        },
    }
}

/// Extracts participant speaker.
fn extract_participant_speaker(input: &str) -> Option<&str> {
    let line = input
        .lines()
        .find(|l| l.trim_start().starts_with("@Participants:"))?;
    let (_, rest) = line.split_once(':')?;
    rest.split_whitespace().next()
}

/// Generate Rust test for a construct example
pub fn generate_construct_test(
    example: &ConstructExample,
    level: &str,
    test_error_path: &str,
) -> String {
    let wrapped = wrap_for_chat_file_parse(example, level);
    format!(
        r#"#[test]
/// Tests expected behavior.
fn test_{name}() -> Result<(), {test_error_path}> {{
    let parser = TreeSitterParser::new()?;
    let _parsed = parser.parse_chat_file({wrapped_input:?})?;

    Ok(())
}}

"#,
        name = example.test_name(),
        wrapped_input = wrapped,
    )
}

/// Generate Rust test for an error example
pub fn generate_error_test(
    error: &ErrorDefinition,
    example: &ErrorExample,
    test_error_path: &str,
    error_type: &str,
    source_file: &str,
    index: usize,
    status: &str,
) -> String {
    let sanitized_source = source_file
        .strip_suffix(".md")
        .unwrap_or(source_file)
        .replace(['.', '-', ' '], "_")
        .to_lowercase();

    let ignore_attr = if status == "not_implemented" {
        format!("#[ignore = \"Status: not_implemented ({})\"]", error.code)
    } else {
        String::new()
    };

    // Build test function name, avoiding double underscores when sanitized_name is empty
    let name = example.sanitized_name();
    let fn_suffix = if name.is_empty() {
        format!("{index}")
    } else {
        format!("{name}_{index}")
    };

    if error_type == "validation" {
        let codes = if example.expected_codes.is_empty() {
            vec![error.code.clone()]
        } else {
            example.expected_codes.clone()
        };

        format!(
            r#"{ignore_attr}
/// Tests expected behavior.
#[test]
fn test_{sanitized_source}_{fn_suffix}() -> Result<(), {test_error_path}> {{
    let parser = TreeSitterParser::new()?;
    let sink = talkbank_model::ErrorCollector::new();
    let mut chat_file = parser.parse_chat_file_streaming({input:?}, &sink);

    // Run validation
    chat_file.validate_with_alignment(&sink, None);

    let errors = sink.into_vec();
    let expected_codes = vec![{expected_codes}];

    for code in expected_codes {{
        let expected = talkbank_model::ErrorCode::new(code);
        let has_expected = errors.iter().any(|err| err.code == expected);
        assert!(has_expected, "Expected error code {{}}, but got: {{:?}}",
            code, errors.iter().map(|err| err.code.as_str()).collect::<Vec<_>>());
    }}

    Ok(())
}}

"#,
            ignore_attr = ignore_attr,
            sanitized_source = sanitized_source,
            fn_suffix = fn_suffix,
            input = example.input,
            expected_codes = codes
                .iter()
                .map(|c| format!("{:?}", c))
                .collect::<Vec<_>>()
                .join(", "),
        )
    } else {
        let codes = if example.expected_codes.is_empty() {
            vec![error.code.clone()]
        } else {
            example.expected_codes.clone()
        };

        format!(
            r#"{ignore_attr}
/// Tests expected behavior.
#[test]
fn test_{sanitized_source}_{fn_suffix}() -> Result<(), {test_error_path}> {{
    let parser = TreeSitterParser::new()?;
    let result = parser.parse_{context}({input:?});

    let errors = match result {{
        Ok(_) => return Err({test_error_path}::Failure("Expected parse error but parsing succeeded".to_string())),
        Err(errors) => errors,
    }};

    let expected_codes = vec![{expected_codes}];
    for code in expected_codes {{
        let expected = talkbank_model::ErrorCode::new(code);
        let has_expected = errors.errors.iter().any(|err| err.code == expected);
        assert!(has_expected, "Expected error code {{}}, but got: {{:?}}",
            code, errors.errors.iter().map(|err| err.code.as_str()).collect::<Vec<_>>());
    }}

    Ok(())
}}

"#,
            ignore_attr = ignore_attr,
            sanitized_source = sanitized_source,
            fn_suffix = fn_suffix,
            context = example.context,
            input = example.input,
            expected_codes = codes
                .iter()
                .map(|c| format!("{:?}", c))
                .collect::<Vec<_>>()
                .join(", "),
        )
    }
}

/// Generate just the test bodies (no imports) for construct specs
pub fn generate_construct_test_body(specs: &[ConstructSpec], test_error_path: &str) -> String {
    let mut output = String::new();

    output.push_str("// Generated by gen_rust_tests - test bodies only\n");
    output.push_str("// DO NOT EDIT MANUALLY - regenerate from talkbank-tools spec\n\n");

    for spec in specs {
        for example in &spec.examples {
            output.push_str(&generate_construct_test(
                example,
                &spec.metadata.level,
                test_error_path,
            ));
        }
    }

    output
}

/// Generate a complete Rust test file from construct specs
pub fn generate_construct_test_file(specs: &[ConstructSpec], test_error_path: &str) -> String {
    let mut output = String::new();

    output.push_str(
        r#"// Generated by gen_rust_tests
// DO NOT EDIT MANUALLY - regenerate from talkbank-tools spec

use talkbank_parser::TreeSitterParser;

"#,
    );

    output.push_str(&generate_construct_test_body(specs, test_error_path));

    output
}

/// Generate just the test bodies (no imports) for error specs
pub fn generate_error_test_body(specs: &[ErrorSpec], test_error_path: &str) -> String {
    let mut output = String::new();

    output.push_str("// Generated by gen_rust_tests - test bodies only\n");
    output.push_str("// DO NOT EDIT MANUALLY - regenerate from talkbank-tools spec\n\n");

    for spec in specs {
        for error in &spec.errors {
            for (i, example) in error.examples.iter().enumerate() {
                output.push_str(&generate_error_test(
                    error,
                    example,
                    test_error_path,
                    &spec.metadata.error_type,
                    &spec.source_file,
                    i,
                    &spec.metadata.status,
                ));
            }
        }
    }

    output
}

/// Generate a complete Rust test file from error specs
pub fn generate_error_test_file(specs: &[ErrorSpec], test_error_path: &str) -> String {
    let mut output = String::new();

    output.push_str(
        r#"// Generated by gen_rust_tests
// DO NOT EDIT MANUALLY - regenerate from talkbank-tools spec

use talkbank_model::ErrorSink;

"#,
    );

    output.push_str(&generate_error_test_body(specs, test_error_path));

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::construct::*;

    /// Tests generate construct test.
    #[test]
    fn test_generate_construct_test() {
        let example = ConstructExample {
            name: "simple_word".to_string(),
            input: "hello".to_string(),
            description: "Plain word".to_string(),
            expected: ExpectedParseTree {
                cst: "(word\n  (segment))".to_string(),
                wrapped_input: None,
                full_cst: None,
            },
            input_type: "standalone_word".to_string(),
        };

        let output =
            generate_construct_test(&example, "word", "talkbank_utils::test_error::TestError");
        assert!(output.contains("fn test_simple_word"));
        assert!(output.contains("parse_chat_file"));
        assert!(output.contains("Result"));
    }

    /// Tests wrap chat fragment in file context.
    #[test]
    fn test_wrap_chat_fragment_in_file_context() {
        let example = ConstructExample {
            name: "overlap_points".to_string(),
            input: "*CHI:\t⌈0 &=laughter⌉ .".to_string(),
            description: "chat fragment".to_string(),
            expected: ExpectedParseTree {
                cst: String::new(),
                wrapped_input: None,
                full_cst: None,
            },
            input_type: "chat".to_string(),
        };

        let wrapped = wrap_for_chat_file_parse(&example, "main_tier");
        assert!(wrapped.contains("@Begin"));
        assert!(wrapped.contains("@ID:\teng|corpus|CHI"));
        assert!(wrapped.contains("*CHI:\t⌈0 &=laughter⌉ ."));
        assert!(wrapped.contains("@End"));
    }

    /// Tests wrap participants header with matching id.
    #[test]
    fn test_wrap_participants_header_with_matching_id() {
        let example = ConstructExample {
            name: "participants_single".to_string(),
            input: "@Participants:\tMOT Mother".to_string(),
            description: "header fragment".to_string(),
            expected: ExpectedParseTree {
                cst: String::new(),
                wrapped_input: None,
                full_cst: None,
            },
            input_type: "participants_header".to_string(),
        };

        let wrapped = wrap_for_chat_file_parse(&example, "header");
        assert!(wrapped.contains("@Participants:\tMOT Mother"));
        assert!(wrapped.contains("@ID:\teng|corpus|MOT"));
    }
}
