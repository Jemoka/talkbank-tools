//! Data models for validation TUI.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use std::path::PathBuf;
use std::sync::Arc;
use talkbank_model::{LineMap, ParseError};

/// A file with its associated validation errors.
#[derive(Debug, Clone)]
pub struct FileErrors {
    /// Path to the validated CHAT file.
    pub path: PathBuf,
    /// Validation and parse errors associated with this file.
    pub errors: Vec<ParseError>,
    /// Full source text for calculating line/column if needed (shared reference)
    pub source: Arc<str>,
}

impl FileErrors {
    /// Ensure all errors have line/column information calculated.
    pub fn ensure_line_columns(&mut self) {
        // Build LineMap once for O(log n) lookups instead of O(n) per error
        let line_map = LineMap::new(&self.source);
        for error in &mut self.errors {
            if error.location.line.is_none() || error.location.column.is_none() {
                let (line_0, col_0) = line_map.line_col_of(error.location.span.start);
                error.location.line = Some(line_0 + 1);
                error.location.column = Some(col_0 + 1);
            }
        }
    }
}
