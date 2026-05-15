//! `ParseError` → LSP `Diagnostic` conversion.
//!
//! Converts `talkbank-model` parse errors into LSP diagnostics with correct
//! ranges (via [`LineIndex`]), severity mapping, error code URLs, related
//! information spans, and suggestion data. This is the single conversion point
//! used by both the full-file and incremental validation paths.

use super::super::utils::LineIndex;
use super::related_info::compute_related_information;
use talkbank_model::ParseError;
use talkbank_model::model::ChatFile;
use tower_lsp::lsp_types::*;

/// Convert a `ParseError` to an LSP `Diagnostic`, using a prebuilt line index.
fn to_diagnostic_indexed(
    error: &ParseError,
    text: &str,
    index: &LineIndex,
    uri: Option<&Url>,
    chat_file: Option<&ChatFile>,
) -> Diagnostic {
    let start_pos = index.offset_to_position(text, error.location.span.start);
    let end_pos = index.offset_to_position(text, error.location.span.end);
    build_diagnostic(error, text, start_pos, end_pos, uri, chat_file)
}

/// Builds diagnostic for downstream use.
fn build_diagnostic(
    error: &ParseError,
    text: &str,
    start_pos: Position,
    end_pos: Position,
    uri: Option<&Url>,
    chat_file: Option<&ChatFile>,
) -> Diagnostic {
    // Every diagnostic is an Error (Severity::Warning is reserved in
    // the enum for compatibility with miette's severity levels but is
    // no longer emitted anywhere in the validation layer — see the
    // 2026-04-22 depfile.cut conformance sweep). If a future caller
    // downgrades a specific code to Warning via `ValidationConfig`, we
    // surface it as an LSP warning; everything else is an error.
    let severity = match error.severity {
        talkbank_model::Severity::Error => DiagnosticSeverity::ERROR,
        talkbank_model::Severity::Warning => DiagnosticSeverity::WARNING,
    };

    let range = Range {
        start: start_pos,
        end: end_pos,
    };

    // Compute related information based on error code and available context
    let related_information = compute_related_information(error, text, uri, chat_file, range);

    let tags = diagnostic_tags(error);

    Diagnostic {
        range,
        severity: Some(severity),
        code: Some(NumberOrString::String(error.code.to_string())),
        code_description: error
            .help_url
            .as_ref()
            .and_then(|url| Url::parse(url).ok().map(|href| CodeDescription { href })),
        source: Some("talkbank".to_string()),
        message: error.message.clone(),
        related_information,
        tags,
        ..Default::default()
    }
}

/// Map error codes to LSP diagnostic tags for visual treatment.
///
/// `UNNECESSARY` causes the editor to fade out the marked range, which is
/// appropriate for empty or redundant content that can safely be removed.
fn diagnostic_tags(error: &ParseError) -> Option<Vec<DiagnosticTag>> {
    use talkbank_model::ErrorCode;
    match error.code {
        // Empty content that could be removed.
        ErrorCode::EmptyUtterance | ErrorCode::EmptyColon => Some(vec![DiagnosticTag::UNNECESSARY]),
        _ => None,
    }
}

/// Convert a batch of parse/validation errors into LSP diagnostics while reusing a single line index.
///
/// The CHAT manual describes the File Format and Main Tier tokens that each diagnostic references.
/// This helper lets us convert multiple errors at once without reconstructing the line index per error, making
/// bulk diagnostics affordable when validating a full document.
pub fn to_diagnostics_batch(errors: &[&ParseError], text: &str) -> Vec<Diagnostic> {
    let index = LineIndex::new(text);
    errors
        .iter()
        .map(|e| to_diagnostic_indexed(e, text, &index, None, None))
        .collect()
}

/// Convert a batch of errors to diagnostics with optional URI and ChatFile context.
///
/// Providing `uri` and `chat_file` allows the helper to attach related-information links and helper spans that refer
/// to the same tier/line numbers described in the manual's Headers and Tier sections.
pub fn to_diagnostics_batch_with_context(
    errors: &[&ParseError],
    text: &str,
    uri: Option<&Url>,
    chat_file: Option<&ChatFile>,
) -> Vec<Diagnostic> {
    let index = LineIndex::new(text);
    errors
        .iter()
        .map(|e| to_diagnostic_indexed(e, text, &index, uri, chat_file))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::{ErrorCode, Severity, Span};

    fn make_error(code: ErrorCode, severity: Severity, start: u32, end: u32) -> ParseError {
        ParseError::at_span(code, severity, Span { start, end }, format!("Test: {code}"))
    }

    #[test]
    fn severity_mapping() {
        // Pure mapping test: Error→ERROR, Warning→WARNING. No
        // validator emits Warning in the current codebase (see the
        // 2026-04-22 depfile.cut conformance sweep that flipped every
        // soft-accept to a hard Error), but the enum variant is
        // retained for future use and this test locks in the LSP
        // mapping so that if/when a validator *does* emit Warning
        // again, the LSP surfaces it correctly.
        let text = "*CHI:\thello .\n";
        let err = make_error(ErrorCode::MissingTerminator, Severity::Error, 0, 5);
        let warn = make_error(ErrorCode::EmptyUtterance, Severity::Warning, 0, 5);

        let diags = to_diagnostics_batch(&[&err, &warn], text);
        assert_eq!(diags[0].severity, Some(DiagnosticSeverity::ERROR));
        assert_eq!(diags[1].severity, Some(DiagnosticSeverity::WARNING));
    }

    #[test]
    fn error_code_is_string() {
        let text = "*CHI:\thello .\n";
        let err = make_error(ErrorCode::MissingTerminator, Severity::Error, 0, 5);
        let diags = to_diagnostics_batch(&[&err], text);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("E305".to_string()))
        );
    }

    #[test]
    fn empty_utterance_gets_unnecessary_tag() {
        let err = make_error(ErrorCode::EmptyUtterance, Severity::Error, 0, 5);
        let tags = diagnostic_tags(&err);
        assert!(tags.is_some());
        assert!(tags.unwrap().contains(&DiagnosticTag::UNNECESSARY));
    }

    #[test]
    fn normal_error_has_no_tags() {
        let err = make_error(ErrorCode::MissingTerminator, Severity::Error, 0, 5);
        let tags = diagnostic_tags(&err);
        assert!(tags.is_none());
    }

    #[test]
    fn batch_converts_multiple_errors() {
        let text = "@UTF8\n@Begin\n*CHI:\thello .\n@End\n";
        let e1 = make_error(ErrorCode::MissingTerminator, Severity::Error, 12, 20);
        let e2 = make_error(ErrorCode::EmptyUtterance, Severity::Error, 12, 20);
        let diags = to_diagnostics_batch(&[&e1, &e2], text);
        assert_eq!(diags.len(), 2);
        assert_eq!(diags[0].source, Some("talkbank".to_string()));
        assert_eq!(diags[1].source, Some("talkbank".to_string()));
    }

    #[test]
    fn range_positions_are_correct() {
        let text = "line0\nline1\nline2\n";
        let err = make_error(ErrorCode::MissingTerminator, Severity::Error, 6, 11);
        let diags = to_diagnostics_batch(&[&err], text);
        assert_eq!(diags[0].range.start.line, 1); // "line1" starts at byte 6
        assert_eq!(diags[0].range.start.character, 0);
        assert_eq!(diags[0].range.end.line, 1);
        assert_eq!(diags[0].range.end.character, 5);
    }
}
