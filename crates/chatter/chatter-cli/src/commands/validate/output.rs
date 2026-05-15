//! Output formatting for single-file validation results (text and JSON modes).
//!
//! Shared by both the single-file and directory validation paths. JSON output
//! includes file path, status, error count, and per-error details (code, severity,
//! message, span) so CI pipelines can parse results without screen-scraping.
//! Text mode delegates to [`print_errors`](crate::output::print_errors) for
//! miette-style diagnostics, and adds a `(cached)` suffix when the result came
//! from the on-disk cache.

use std::fs;
use std::path::PathBuf;

use crate::cli::OutputFormat;
use crate::output::{CASCADING_HINT, print_errors, should_show_cascading_hint};
use talkbank_model::ParseError;

/// Format validation results according to the requested output style (JSON or human-readable text).
///
/// JSON output obeys the schema documented in the CLI's audit section of the CHAT manual, reporting
/// file path, error count, and a detailed list of diagnostics with spans. Text mode reuses the shared
/// `print_errors` helper and optionally suppresses output when `quiet` is enabled or the result was
/// loaded from cache. The `cached` flag mirrors the CLI’s decision to skip revalidation when a
/// previous run already reported success for the same file and options described under File Format.
pub(super) fn output_validation_result(
    path: &PathBuf,
    errors: &[ParseError],
    source: Option<&str>,
    format: OutputFormat,
    cached: bool,
    quiet: bool,
) {
    if matches!(format, OutputFormat::Json) {
        let json_errors: Vec<_> = errors
            .iter()
            .map(|e| {
                serde_json::json!({
                    "code": e.code.to_string(),
                    "severity": format!("{:?}", e.severity),
                    "message": e.message,
                    "location": {
                        "start": e.location.span.start,
                        "end": e.location.span.end,
                    }
                })
            })
            .collect();

        let mut json_output = serde_json::json!({
            "file": path.to_string_lossy(),
            "status": if errors.is_empty() { "valid" } else { "invalid" },
            "error_count": errors.len(),
            "errors": json_errors
        });

        if cached {
            json_output["cached"] = serde_json::json!(true);
        }

        if should_show_cascading_hint(errors) {
            json_output["note"] = serde_json::json!(
                "Some additional checks may not have run because of structural errors. Fix the structural errors first, then re-validate."
            );
        }

        match serde_json::to_string_pretty(&json_output) {
            Ok(serialized) => println!("{}", serialized),
            Err(err) => eprintln!("Error serializing JSON output: {}", err),
        }
    } else {
        if errors.is_empty() {
            if !quiet {
                let suffix = if cached { " (cached)" } else { "" };
                println!("✓ {} is valid{}", path.display(), suffix);
            }
            return;
        }

        if let Some(src) = source {
            print_errors(path, src, errors);
        } else {
            match fs::read_to_string(path) {
                Ok(content) => print_errors(path, &content, errors),
                Err(err) => eprintln!("Error reading {}: {}", path.display(), err),
            }
        }

        if !quiet && should_show_cascading_hint(errors) {
            eprintln!("{}", CASCADING_HINT);
        }
    }
}
