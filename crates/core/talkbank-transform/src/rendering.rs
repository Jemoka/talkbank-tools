//! Error rendering functions using miette
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use miette::{GraphicalReportHandler, GraphicalTheme, NamedSource, Report};
use std::fmt::Write as _;
use std::sync::Arc;
use talkbank_model::ParseError;

/// Render a ParseError using miette for beautiful diagnostics.
pub fn render_error_with_miette(error: &ParseError) -> String {
    let mut output = String::new();
    let handler = GraphicalReportHandler::new_themed(GraphicalTheme::unicode());

    if let Err(_e) = handler.render_report(&mut output, error) {
        // Fallback if miette rendering fails
        write!(&mut output, "{}", error).ok();
    }

    output
}

/// Render a ParseError using miette with a shared source buffer (no error mutation).
///
/// Uses `{:?}` (Debug) formatting which delegates to miette's installed handler.
/// This respects terminal color detection — produces ANSI codes when stderr is a
/// terminal, plain text otherwise. For forced ANSI output regardless of terminal,
/// use [`render_error_with_miette_with_source_colored`].
pub fn render_error_with_miette_with_source(
    error: &ParseError,
    source_name: &str,
    source: &str,
) -> String {
    let mut output = String::new();
    let named_source = NamedSource::new(source_name, source.to_string());
    let report = Report::new(error.clone()).with_source_code(named_source);

    if let Err(_e) = write!(&mut output, "{:?}", report) {
        write!(&mut output, "{}", error).ok();
    }

    output
}

/// Render a ParseError with miette, forcing ANSI color output regardless of terminal.
///
/// Used by the desktop app (Tauri) where output is converted to HTML, not displayed
/// in a terminal. The standard `render_error_with_miette_with_source` would produce
/// uncolored output because miette detects no terminal.
pub fn render_error_with_miette_with_source_colored(
    error: &ParseError,
    source_name: &str,
    source: &str,
) -> String {
    let mut output = String::new();
    let handler = GraphicalReportHandler::new_themed(GraphicalTheme::unicode())
        .with_links(false)
        .with_footer(String::new());

    // Create a wrapper that attaches the source code to the error
    let named_source = NamedSource::new(source_name, source.to_string());
    let wrapper = SourcedError {
        error: error.clone(),
        source: named_source,
    };

    if let Err(_e) = handler.render_report(&mut output, &wrapper) {
        write!(&mut output, "{}", error).ok();
    }

    output
}

/// Wrapper that attaches a `NamedSource` to a `ParseError` for miette rendering.
///
/// `ParseError` implements `Diagnostic` but its `source_code()` may return `None`
/// if the error was created without embedded source (which is the normal case for
/// streamed validation events). This wrapper provides the source.
struct SourcedError {
    error: ParseError,
    source: NamedSource<String>,
}

impl std::fmt::Debug for SourcedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.error, f)
    }
}

impl std::fmt::Display for SourcedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.error, f)
    }
}

impl std::error::Error for SourcedError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.error.source()
    }
}

impl miette::Diagnostic for SourcedError {
    fn code<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        self.error.code()
    }

    fn severity(&self) -> Option<miette::Severity> {
        self.error.severity()
    }

    fn help<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        self.error.help()
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = miette::LabeledSpan> + '_>> {
        self.error.labels()
    }

    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        Some(&self.source)
    }
}

/// Render a ParseError using miette with a shared NamedSource.
pub fn render_error_with_miette_with_named_source(
    error: &ParseError,
    source: &NamedSource<Arc<String>>,
) -> String {
    let mut output = String::new();
    let report = Report::new(error.clone()).with_source_code(source.clone());

    if let Err(_e) = write!(&mut output, "{:?}", report) {
        write!(&mut output, "{}", error).ok();
    }

    output
}
