//! Validates error specifications against actual parser + validator behavior.
//!
//! Detects error code mismatches: specs whose CHAT examples do not produce the
//! claimed error code. This catches auto-generated specs with wrong examples.
//!
//! ## Usage
//!
//! ```bash
//! # Full validation (error code match)
//! cargo run --bin validate_error_specs --manifest-path crates/spec/talkbank-spec-testrun/Cargo.toml -- --check-codes
//!
//! # Include not_implemented/deprecated specs
//! cargo run --bin validate_error_specs --manifest-path crates/spec/talkbank-spec-testrun/Cargo.toml -- --check-codes --include-skipped
//!
//! # Only check specific codes
//! cargo run --bin validate_error_specs --manifest-path crates/spec/talkbank-spec-testrun/Cargo.toml -- --check-codes --filter E248,E249
//! ```
//!
//! Returns exit code 0 if all specs are valid, 1 otherwise.

use clap::Parser;
use talkbank_spec_testgen::spec::error::ErrorSpec;
use std::path::PathBuf;
use talkbank_model::ErrorCollector;
use talkbank_parser::TreeSitterParser;

/// CLI arguments for the error spec validator.
#[derive(Parser)]
#[command(name = "validate_error_specs")]
#[command(about = "Validate error specifications against actual parser + validator behavior")]
struct Args {
    /// Root directory containing error specs.
    #[arg(short, long, default_value = "resources/spec/errors")]
    spec_dir: PathBuf,

    /// Verify that each example produces the claimed error code.
    #[arg(long)]
    check_codes: bool,

    /// Include not_implemented/deprecated specs (normally skipped).
    #[arg(long)]
    include_skipped: bool,

    /// Comma-separated list of error codes to check (e.g., "E248,E249").
    /// If omitted, checks all specs.
    #[arg(long, value_delimiter = ',')]
    filter: Option<Vec<String>>,
}

/// Outcome for a single example within a spec.
enum ExampleOutcome {
    /// Codes match (or code checking is off).
    Pass,
    /// Spec is skipped (not_implemented / deprecated / no expected codes).
    Skipped(String),
    /// Error code not found in actual output.
    CodeMismatch {
        expected: Vec<String>,
        actual: Vec<String>,
    },
}

fn main() {
    let args = Args::parse();

    eprintln!(
        "Validating error specifications from: {}\n",
        args.spec_dir.display()
    );

    match validate_all_specs(&args) {
        Ok(()) => std::process::exit(0),
        Err(_) => std::process::exit(1),
    }
}

/// Validate all error specifications in a directory.
fn validate_all_specs(args: &Args) -> Result<(), String> {
    let specs =
        ErrorSpec::load_all(&args.spec_dir).map_err(|e| format!("Failed to load specs: {}", e))?;

    if specs.is_empty() {
        eprintln!("Warning: No specs found in {}", args.spec_dir.display());
        return Ok(());
    }

    eprintln!("Found {} spec files\n", specs.len());

    let parser = TreeSitterParser::new().map_err(|e| format!("Failed to create parser: {}", e))?;

    let mut code_mismatches: Vec<String> = Vec::new();
    let mut passed = 0u32;
    let mut skipped = 0u32;
    let mut total = 0u32;

    for spec in &specs {
        let code = match spec.errors.first() {
            Some(e) => e.code.as_str(),
            None => {
                eprintln!(
                    "  WARN  {} has no error definitions, skipping",
                    spec.source_file
                );
                continue;
            }
        };

        if let Some(ref filter) = args.filter {
            if !filter.iter().any(|f| f == code) {
                continue;
            }
        }

        let status = &spec.metadata.status;

        for (ex_idx, error_def) in spec.errors.iter().enumerate() {
            for (input_idx, example) in error_def.examples.iter().enumerate() {
                total += 1;

                let label = if error_def.examples.len() > 1 {
                    format!("{} (example {})", code, input_idx + 1)
                } else if spec.errors.len() > 1 {
                    format!("{} (def {})", code, ex_idx + 1)
                } else {
                    code.to_string()
                };

                let outcome = validate_example(
                    &parser,
                    status,
                    example,
                    args.check_codes,
                    args.include_skipped,
                );

                match outcome {
                    ExampleOutcome::Pass => {
                        passed += 1;
                        eprintln!("  PASS  {}", label);
                    }
                    ExampleOutcome::Skipped(reason) => {
                        skipped += 1;
                        eprintln!("  SKIP  {} ({})", label, reason);
                    }
                    ExampleOutcome::CodeMismatch { expected, actual } => {
                        code_mismatches.push(format!(
                            "  CODE   {}: expected {:?}, got {:?}",
                            label, expected, actual
                        ));
                    }
                }
            }
        }
    }

    eprintln!();

    if !code_mismatches.is_empty() {
        eprintln!("CODE MISMATCHES ({}):\n", code_mismatches.len());
        for msg in &code_mismatches {
            eprintln!("{}", msg);
        }
        eprintln!();
    }

    eprintln!(
        "Summary: {} passed, {} skipped, {} errors (of {} total)",
        passed,
        skipped,
        code_mismatches.len(),
        total,
    );

    if code_mismatches.is_empty() {
        eprintln!("\nAll checked specs valid.\n");
        Ok(())
    } else {
        eprintln!("\nValidation failed.\n");
        Err("Validation failed".to_string())
    }
}

/// Validate a single example against the parser + validator.
fn validate_example(
    parser: &TreeSitterParser,
    status: &str,
    example: &talkbank_spec_testgen::spec::error::ErrorExample,
    check_codes: bool,
    include_skipped: bool,
) -> ExampleOutcome {
    if !include_skipped && (status == "not_implemented" || status == "deprecated") {
        return ExampleOutcome::Skipped(format!("status: {}", status));
    }

    if check_codes && example.expected_codes.is_empty() {
        return ExampleOutcome::Skipped("no Expected Error Codes line".to_string());
    }

    // Run parse + validate. If a spec example triggers a panic (e.g., E245's
    // lone stress marker hitting NonEmptyString::new_unchecked), report it as
    // a code mismatch with "PANIC" rather than crashing the entire tool.
    let all_errors = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let sink = ErrorCollector::new();
        let mut chat_file = parser.parse_chat_file_streaming(&example.input, &sink);
        chat_file.validate_with_alignment(&sink, None);
        sink.into_vec()
    })) {
        Ok(errors) => errors,
        Err(payload) => {
            let msg = payload
                .downcast_ref::<String>()
                .map(|s| s.as_str())
                .or_else(|| payload.downcast_ref::<&str>().copied())
                .unwrap_or("unknown panic");
            eprintln!("  PANIC: {}", msg);
            return ExampleOutcome::CodeMismatch {
                expected: example.expected_codes.clone(),
                actual: vec!["PANIC".to_string()],
            };
        }
    };

    if !check_codes {
        return ExampleOutcome::Pass;
    }

    let mut actual_codes: Vec<String> = all_errors
        .iter()
        .map(|e| e.code.as_str().to_string())
        .collect();
    actual_codes.sort();
    actual_codes.dedup();

    let all_expected_present = example
        .expected_codes
        .iter()
        .all(|expected| actual_codes.iter().any(|actual| actual == expected));

    if all_expected_present {
        ExampleOutcome::Pass
    } else {
        ExampleOutcome::CodeMismatch {
            expected: example.expected_codes.clone(),
            actual: actual_codes,
        }
    }
}
