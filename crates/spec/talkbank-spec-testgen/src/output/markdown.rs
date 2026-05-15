//! # Markdown Documentation Generator
//!
//! Generates publishable error documentation in Markdown format.
//!
//! Each generated page surfaces the implementation status from the source
//! spec (`resources/spec/errors/*.md`) as a visible badge so researchers can tell at a
//! glance whether the validator actually enforces the documented check.

use crate::spec::error::{ErrorDefinition, ErrorSpec};

/// Render the short badge label shown after the Severity line.
///
/// Accepts the raw `Status` string as parsed from the spec metadata
/// (`"implemented"` or `"not_implemented"`). Unknown values pass through
/// unchanged so that spec authors notice typos in generated docs.
fn status_badge(status: &str) -> &str {
    match status {
        "implemented" => "✅ Active",
        "not_implemented" => "⏳ Planned",
        other => other,
    }
}

/// Render the callout that explains the badge meaning to readers.
///
/// Lives immediately under the title so researchers scanning the docs see
/// the enforcement state before any other metadata.
fn status_callout(status: &str) -> &'static str {
    match status {
        "implemented" => "This check is active in the validator.\n\n",
        "not_implemented" => "This check is documented but not yet enforced by the validator. The error code will not fire until implementation is complete.\n\n",
        _ => "\n\n",
    }
}

/// Generate a Markdown page for a single error.
///
/// `status` comes from the owning `ErrorSpec`'s metadata (see
/// [`crate::spec::error::ErrorMetadata::status`]). It is passed explicitly
/// because `ErrorDefinition` does not carry category-level metadata.
pub fn generate_error_page(error: &ErrorDefinition, status: &str) -> String {
    let mut output = String::new();
    let badge = status_badge(status);

    // Title
    output.push_str(&format!("# {}: {}\n\n", error.code, error.name));

    // Status callout (blockquote) placed before severity so it is the first
    // operational fact the reader sees.
    output.push_str(&format!("> {} — ", badge));
    output.push_str(status_callout(status));

    // Metadata
    output.push_str(&format!("**Severity**: {}\n\n", error.severity));
    output.push_str(&format!("**Status**: {}\n\n", badge));

    // Description
    output.push_str("## Description\n\n");
    output.push_str(&format!("{}\n\n", error.description));

    // Examples
    if !error.examples.is_empty() {
        output.push_str("## Examples\n\n");
        for (i, example) in error.examples.iter().enumerate() {
            output.push_str(&format!("### Example {}\n\n", i + 1));
            output.push_str("```chat\n");
            output.push_str(&example.input);
            output.push_str("\n```\n\n");
            output.push_str(&format!("**Error**: {}\n\n", example.expected_message));
        }
    }

    // How to fix
    output.push_str("## How to Fix\n\n");
    output.push_str(&format!("{}\n\n", error.suggestion));

    // Help URL
    if let Some(url) = &error.help_url {
        output.push_str("## More Information\n\n");
        output.push_str(&format!("[CHAT Manual]({})\n\n", url));
    }

    output
}

/// Generate index page for all errors.
///
/// The Status column uses a compact icon (✅ / ⏳) so category tables stay
/// readable. The per-page view spells out the full badge.
pub fn generate_error_index(specs: &[ErrorSpec]) -> String {
    let mut output = String::new();

    output.push_str("# CHAT Error Reference\n\n");
    output.push_str("Complete reference for all CHAT parser and validation errors.\n\n");
    output.push_str(
        "Status legend: ✅ = active in the validator, ⏳ = documented but not yet enforced.\n\n",
    );

    // Group by category
    for spec in specs {
        output.push_str(&format!(
            "## {} ({})\n\n",
            spec.metadata.category, spec.metadata.range
        ));
        output.push_str(&format!("{}\n\n", spec.metadata.description));

        output.push_str("| Code | Name | Severity | Status |\n");
        output.push_str("|------|------|----------|--------|\n");

        // Status is attached to the spec (category), not each ErrorDefinition,
        // so all errors in a spec share the same icon.
        let status_icon = match spec.metadata.status.as_str() {
            "implemented" => "✅",
            "not_implemented" => "⏳",
            _ => "?",
        };

        for error in &spec.errors {
            output.push_str(&format!(
                "| [{}]({}.md) | {} | {} | {} |\n",
                error.code, error.code, error.name, error.severity, status_icon
            ));
        }

        output.push('\n');
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::error::*;

    /// Active-status pages should advertise themselves as enforced.
    #[test]
    fn test_generate_error_page_active() {
        let error = ErrorDefinition {
            code: "E241".to_string(),
            name: "IllegalUntranscribed".to_string(),
            severity: "error".to_string(),
            description: "Word contains illegal untranscribed marker".to_string(),
            suggestion: "Use 'xxx' for unintelligible speech".to_string(),
            help_url: Some("https://talkbank.org/errors/E241".to_string()),
            references: ErrorReference::default(),
            examples: vec![],
        };

        let output = generate_error_page(&error, "implemented");
        assert!(output.contains("# E241"));
        assert!(output.contains("IllegalUntranscribed"));
        assert!(output.contains("**Severity**: error"));
        assert!(output.contains("**Status**: ✅ Active"));
        assert!(output.contains("> ✅ Active — This check is active in the validator."));
    }

    /// Not-implemented specs should be clearly marked as planned so readers
    /// do not expect runtime enforcement.
    #[test]
    fn test_generate_error_page_planned() {
        let error = ErrorDefinition {
            code: "E321".to_string(),
            name: "SomePlannedCheck".to_string(),
            severity: "error".to_string(),
            description: "Planned check".to_string(),
            suggestion: "TBD".to_string(),
            help_url: None,
            references: ErrorReference::default(),
            examples: vec![],
        };

        let output = generate_error_page(&error, "not_implemented");
        assert!(output.contains("**Status**: ⏳ Planned"));
        assert!(output.contains("> ⏳ Planned — "));
        assert!(output.contains("not yet enforced by the validator"));
    }
}
