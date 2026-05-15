//! Miette-backed diagnostics for tests.

use std::path::Path;

use talkbank_model::ParseError;
use talkbank_transform::PipelineError;
use talkbank_transform::render_error_with_miette;

/// Print parsing/validation errors with miette-style formatting.
pub fn print_pipeline_error(path: Option<&Path>, source: &str, error: &PipelineError) {
    match error {
        PipelineError::Parse(parse_errors) => {
            print_parse_errors(
                path,
                source,
                &parse_errors.errors,
                "Encountered parse errors",
            );
        }
        PipelineError::Validation(validation_errors) => {
            print_parse_errors(
                path,
                source,
                validation_errors,
                "Encountered validation errors",
            );
        }
        other => {
            let filename = format_filename(path);
            eprintln!("✗ Pipeline error while parsing {}: {}", filename, other);
        }
    }
}

/// Prints parse errors.
fn print_parse_errors(path: Option<&Path>, source: &str, errors: &[ParseError], header: &str) {
    if errors.is_empty() {
        return;
    }

    let mut enhanced = errors.to_vec();
    talkbank_model::enhance_errors_with_source(&mut enhanced, source);

    let filename = format_filename(path);
    eprintln!("{} in {}", header, filename);
    eprintln!("✗ Found {} issue(s)\n", errors.len());

    for error in &enhanced {
        eprintln!("{}", render_error_with_miette(error));
        eprintln!();
    }
}

/// Formats filename.
fn format_filename(path: Option<&Path>) -> String {
    path.map(|p| p.display().to_string())
        .unwrap_or_else(|| "<input>".to_string())
}
