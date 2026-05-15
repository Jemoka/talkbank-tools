//! Output formatting utilities for CLI.
//!
//! These helpers centralize shared output behavior (miette-enhanced errors, progress
//! spinners, json emitters) so commands can call reuse formatting instead of reimplementing it.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use talkbank_model::{ErrorSink, ParseError, enhance_errors_with_source};
use talkbank_transform::render_error_with_miette_with_source;

/// Print errors to stderr with miette formatting
pub fn print_errors(path: &Path, content: &str, errors: &[ParseError]) {
    if !errors.is_empty() {
        eprintln!("✗ Errors found in {}", path.display());
        eprintln!();
    }

    // Clone and enhance errors with source context before rendering
    let mut enhanced_errors = errors.to_vec();
    enhance_errors_with_source(&mut enhanced_errors, content);

    for error in &enhanced_errors {
        let rendered =
            render_error_with_miette_with_source(error, &path.display().to_string(), content);
        eprintln!("{}", rendered);
    }
}

/// Check whether a set of errors contains structural errors (E0xx-E5xx) but no
/// alignment errors (E7xx). When structural errors taint the parse, alignment
/// checks are silently skipped. This helper detects that situation so callers
/// can emit a hint telling the user to fix structural errors first.
pub fn should_show_cascading_hint(errors: &[ParseError]) -> bool {
    let mut has_structural = false;
    let mut has_alignment = false;

    for error in errors {
        let code_str = error.code.as_str();
        // Structural errors: E0xx, E1xx, E2xx, E3xx, E4xx, E5xx
        // Alignment errors: E7xx
        match code_str.as_bytes() {
            [b'E', b'0'..=b'5', ..] => has_structural = true,
            [b'E', b'7', ..] => has_alignment = true,
            _ => {}
        }
    }

    has_structural && !has_alignment
}

/// The cascading error hint text, printed to stderr in text mode.
pub const CASCADING_HINT: &str = "  note: Some additional checks may not have run because of structural errors above.\n        Fix the structural errors first, then re-validate.";

/// ErrorSink that prints errors immediately to the terminal using miette rendering.
pub struct TerminalErrorSink {
    path: PathBuf,
    content: String,
    error_count: AtomicUsize,
    header_printed: AtomicUsize,
}

impl TerminalErrorSink {
    /// Create a terminal sink for one file's content.
    ///
    /// The sink keeps the source text so each streamed error can be enhanced
    /// with line/column context before miette rendering.
    pub fn new(path: &Path, content: &str) -> Self {
        Self {
            path: path.to_path_buf(),
            content: content.to_string(),
            error_count: AtomicUsize::new(0),
            header_printed: AtomicUsize::new(0),
        }
    }

    /// Return the number of errors streamed so far.
    pub fn error_count(&self) -> usize {
        self.error_count.load(Ordering::Relaxed)
    }

    /// Prints single error.
    fn print_single_error(&self, mut error: ParseError) {
        // Print header on first error
        if self
            .header_printed
            .compare_exchange(0, 1, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {
            eprintln!("✗ Errors found in {}", self.path.display());
            eprintln!();
        }

        enhance_errors_with_source(std::slice::from_mut(&mut error), &self.content);
        let rendered = render_error_with_miette_with_source(
            &error,
            &self.path.display().to_string(),
            &self.content,
        );
        eprintln!("{}", rendered);
    }
}

impl ErrorSink for TerminalErrorSink {
    /// Stream one parse error to stderr and increment the counter.
    fn report(&self, error: ParseError) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
        self.print_single_error(error);
    }
}
