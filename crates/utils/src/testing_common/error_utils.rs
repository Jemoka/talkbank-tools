//! Error analysis and comparison utilities.

use talkbank_model::ParseError;

/// Summarize errors into a human-readable string.
pub fn summarize_errors(errors: &[ParseError]) -> String {
    if errors.is_empty() {
        "no errors".to_string()
    } else if errors.len() == 1 {
        format!("{}: {}", errors[0].code.as_str(), errors[0].message)
    } else {
        format!(
            "{} errors (first: {}: {})",
            errors.len(),
            errors[0].code.as_str(),
            errors[0].message
        )
    }
}

/// Compare two error lists, ignoring order but checking all error keys match.
pub fn errors_equal(a: &[ParseError], b: &[ParseError]) -> bool {
    let mut keys_a: Vec<String> = a.iter().map(error_key).collect();
    let mut keys_b: Vec<String> = b.iter().map(error_key).collect();
    keys_a.sort();
    keys_b.sort();
    keys_a == keys_b
}

/// Generate a unique key for an error (for comparison).
pub fn error_key(error: &ParseError) -> String {
    format!(
        "{}:{}:{}..{}:{}",
        error.code.as_str(),
        error.severity,
        error.location.span.start,
        error.location.span.end,
        error.message
    )
}
