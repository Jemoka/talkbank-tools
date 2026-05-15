//! Generate Rust validation test files from error corpus specs
//!
//! This generates tests that:
//! 1. Parse successfully (input is valid grammar)
//! 2. Run validation layer
//! 3. Assert expected validation error code
//!
//! Only generates tests for validation-layer errors (layer = "validation")

use anyhow::Result;
use clap::Parser;
use std::fs;
use std::path::PathBuf;

use generators::spec::error_corpus::ErrorCorpusSpec;

/// CLI arguments: spec directory, output directory for generated test `.rs` files, and fixture directory for `.cha` inputs.
#[derive(Parser)]
#[command(name = "gen_validation_tests")]
#[command(about = "Generate Rust validation test files from error corpus specs")]
struct Args {
    /// Root directory containing error specs
    #[arg(short, long, default_value = "spec/errors")]
    spec_dir: PathBuf,

    /// Output directory for generated test files (e.g., path/to/talkbank-tools/crates/talkbank-parser-tests/tests/generated)
    #[arg(short, long)]
    output_dir: PathBuf,

    /// Fixture directory for test input CHAT files (e.g., path/to/talkbank-tools/crates/talkbank-parser-tests/tests/fixtures/errors)
    #[arg(short, long)]
    fixture_dir: PathBuf,
}

/// Generates Rust validation tests and `.cha` fixture files from validation-layer error specs.
fn main() -> Result<()> {
    let args = Args::parse();

    println!(
        "Loading error corpus specs from: {}",
        args.spec_dir.display()
    );

    let all_specs = ErrorCorpusSpec::load_all(&args.spec_dir)
        .map_err(|e| anyhow::anyhow!("Failed to load error corpus specs: {}", e))?;

    // Filter for validation-layer errors only
    let validation_specs: Vec<_> = all_specs
        .into_iter()
        .filter(|spec| spec.metadata.layer == "validation")
        .collect();

    println!(
        "Found {} validation error spec files",
        validation_specs.len()
    );
    println!();

    // ALWAYS clean generated_validation_*.rs files before regenerating to prevent stale tests
    if args.output_dir.exists() {
        println!("Cleaning old generated validation test files...");
        if let Ok(entries) = fs::read_dir(&args.output_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(filename) = path.file_name() {
                    if let Some(name_str) = filename.to_str() {
                        if has_prefix(name_str, "generated_validation_")
                            && path.extension().is_some_and(|e| e == "rs")
                        {
                            let _ = fs::remove_file(&path);
                        }
                    }
                }
            }
        }
    }

    // Create output and fixture directories
    fs::create_dir_all(&args.output_dir)?;
    fs::create_dir_all(&args.fixture_dir)?;

    // Write fixture files for each test input
    println!("Writing fixture files to: {}", args.fixture_dir.display());
    for spec in &validation_specs {
        for example in &spec.examples {
            let error_code = example.error_code.as_deref().unwrap_or("unknown");
            // Sanitize name: replace spaces, hyphens, and problematic characters with underscores
            let sanitized_name = sanitize_filename(&example.name);
            let fixture_name = format!("{}_{}.cha", error_code, sanitized_name);
            let fixture_path = args.fixture_dir.join(&fixture_name);

            let input = strip_single_trailing_newline(&example.input);
            fs::write(&fixture_path, input)?;
            println!("  ✓ {}", fixture_name);
        }
    }

    // Generate validation test file
    let test_content = generate_validation_test_file(&validation_specs, &args.fixture_dir);
    let test_path = args.output_dir.join("generated_validation_tests.rs");
    fs::write(&test_path, &test_content)?;
    println!("✓ Generated: {}", test_path.display());

    // Generate body version (without imports)
    let body_content = generate_validation_test_body(&validation_specs, &args.fixture_dir);
    let body_path = args.output_dir.join("generated_validation_tests_body.rs");
    fs::write(&body_path, body_content)?;
    println!("✓ Generated: {}", body_path.display());

    println!(
        "\n✓ Generated validation tests to {}",
        args.output_dir.display()
    );
    println!(
        "✓ Generated fixture files to {}",
        args.fixture_dir.display()
    );

    Ok(())
}

/// Sanitize a string for use in filenames
/// Removes or replaces problematic characters that cause issues in file paths
fn sanitize_filename(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            // Replace spaces and hyphens with underscores
            ' ' | '-' => '_',
            // Remove quotes, percent signs, colons, and other problematic chars
            '\'' | '"' | '%' | ':' | '/' | '\\' | '<' | '>' | '|' | '?' | '*' => '_',
            // Keep alphanumeric and underscores
            c if c.is_alphanumeric() || c == '_' => c,
            // Replace anything else with underscore
            _ => '_',
        })
        .collect::<String>()
        // Collapse multiple underscores into one
        .split('_')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

fn has_prefix(text: &str, prefix: &str) -> bool {
    match text.as_bytes().get(0..prefix.len()) {
        Some(slice) => slice == prefix.as_bytes(),
        None => false,
    }
}

fn strip_single_trailing_newline(text: &str) -> String {
    if let Some(stripped) = text.strip_suffix("\r\n") {
        stripped.to_string()
    } else if let Some(stripped) = text.strip_suffix('\n') {
        stripped.to_string()
    } else {
        text.to_string()
    }
}

/// Generate a complete Rust validation test file
fn generate_validation_test_file(
    specs: &[ErrorCorpusSpec],
    fixture_dir: &std::path::Path,
) -> String {
    let mut output = String::new();

    output.push_str(
        r#"// Generated by gen_validation_tests
// DO NOT EDIT MANUALLY - regenerate from talkbank-tools spec
//
// These tests verify that validation layer correctly detects semantic errors
// in input that parses successfully.
//
// Test inputs are loaded from fixture files using include_str!
// See talkbank-tools spec/generators/CLAUDE.md for why we use fixtures instead of embedded literals.

use talkbank_parser::TreeSitterParser;
use talkbank_model::ErrorCollector;
use talkbank_model::ValidationContext;

"#,
    );

    output.push_str(&generate_validation_test_body(specs, fixture_dir));

    output
}

/// Generate just the test bodies (no imports)
fn generate_validation_test_body(
    specs: &[ErrorCorpusSpec],
    fixture_dir: &std::path::Path,
) -> String {
    let mut output = String::new();

    output.push_str("// Generated validation test bodies\n\n");

    for spec in specs {
        for example in &spec.examples {
            output.push_str(&generate_validation_test(
                example,
                &spec.metadata.level,
                fixture_dir,
            ));
        }
    }

    output
}

/// Generate a single validation test
fn generate_validation_test(
    example: &generators::spec::error_corpus::ErrorCorpusExample,
    _level: &str,
    _fixture_dir: &std::path::Path,
) -> String {
    let sanitized_name = sanitize_filename(&example.name);
    let error_code = example.error_code.as_deref().unwrap_or("UNKNOWN");
    let test_name = format!(
        "validation_{}_{}",
        error_code.to_lowercase(),
        sanitized_name.to_lowercase()
    );

    // Fixture file name: E601_example_name.cha (sanitize name to match file creation)
    let fixture_name = format!("{}_{}.cha", error_code, sanitized_name);

    // Relative path from tests/generated/ to tests/fixtures/errors/
    let fixture_path = format!("../fixtures/errors/{}", fixture_name);

    // For validation tests, we ALWAYS parse as a chat file because validation needs
    // file-level context (participants, languages, etc.). The 'level' field indicates
    // WHERE in the file the error is expected (word, tier, header), not what to parse.
    let parse_fn = "parse_chat_file";

    // ChatFile has an instance validate() method that shadows the Validate trait method
    // Use explicit trait syntax to call the Validate trait version
    let validate_call = "Validate::validate(&parsed, &ctx, &errors);";

    format!(
        r#"#[test]
fn test_{test_name}() -> Result<(), talkbank_parser_tests::test_error::TestError> {{
    let parser = TreeSitterParser::new()?;
    let input = include_str!({fixture_path:?});

    // Step 1: Parse should succeed (this is validation error, not parser error)
    let parsed = parser.{parse_fn}(input)?;

    // Step 2: Validation should report error
    let errors = ErrorCollector::new();
    let ctx = ValidationContext::default();
    {validate_call}

    let error_vec = errors.into_vec();
    assert!(!error_vec.is_empty(), "Expected validation error {error_code} but got no errors");

    // Check for expected error code
    let expected_code = talkbank_model::ErrorCode::new({error_code:?});
    let has_expected_error = error_vec.iter().any(|e| e.code == expected_code);
    assert!(has_expected_error,
        "Expected error code {error_code}, but got: {{:?}}",
        error_vec.iter().map(|e| e.code.as_str()).collect::<Vec<_>>()
    );

    Ok(())
}}

"#,
        test_name = test_name,
        fixture_path = fixture_path,
        parse_fn = parse_fn,
        validate_call = validate_call,
        error_code = error_code,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn test_generate_validation_test() -> Result<(), Box<dyn Error>> {
        use generators::spec::error_corpus::ErrorCorpusExample;
        use std::path::PathBuf;

        let example = ErrorCorpusExample {
            name: "illegal_xx".to_string(),
            description: "Illegal untranscribed marker 'xx'".to_string(),
            input: "*CHI:\txx .".to_string(),
            error_code: Some("E241".to_string()),
            error_location: Some("word content".to_string()),
            notes: Some("Should use xxx".to_string()),
            expected_cst: None,
        };

        let fixture_dir = PathBuf::from("tests/fixtures/errors");
        let output = generate_validation_test(&example, "chat_file", &fixture_dir);

        // Verify test structure uses include_str! instead of embedded literal
        assert!(output.contains("fn test_validation_e241_illegal_xx"));
        assert!(output.contains("parse_chat_file"));
        assert!(output.contains("include_str!(\"../fixtures/errors/E241_illegal_xx.cha\")"));
        assert!(output.contains("parse_chat_file"));
        assert!(output.contains("E241"));

        Ok(())
    }
}
