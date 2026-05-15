//! LINES -- add or remove sequential line numbers on CHAT tiers.
//!
//! Reimplements CLAN's LINES command. Since line numbering is a display
//! concern (not structural), the [`LinesCommand`] struct is an AST no-op;
//! the actual logic lives in [`add_line_numbers()`], [`remove_line_numbers()`],
//! and the end-to-end [`run_lines()`] function.
//!
//! Line numbers are formatted as 5-character right-aligned integers prefixed
//! to non-header lines. Header lines (`@Begin`, `@Languages`, etc.) are not
//! numbered.
//!
//! # Differences from CLAN
//!
//! - The AST transform is a no-op; line numbers are added to or removed from
//!   serialized text via `add_line_numbers()` / `remove_line_numbers()` after
//!   the standard parse → serialize round-trip.
//! - This hybrid approach ensures structural integrity from the parse step
//!   while handling the display-only concern of line numbering as a
//!   post-serialization text transformation.

use talkbank_model::ChatFile;

use crate::framework::{TransformCommand, TransformError};

/// Configuration for the LINES command.
pub struct LinesConfig {
    /// If true, remove existing line numbers instead of adding them.
    pub remove: bool,
}

/// LINES transform (AST no-op — use `run_lines` for the actual logic).
pub struct LinesCommand;

impl TransformCommand for LinesCommand {
    type Config = LinesConfig;

    /// AST no-op. Use [`run_lines()`] for the actual line-numbering logic.
    fn transform(&self, _file: &mut ChatFile) -> Result<(), TransformError> {
        // Line numbering is applied as a post-serialization step in the custom
        // run function below. The AST transform is a no-op.
        Ok(())
    }
}

/// Add sequential line numbers to CHAT text lines.
///
/// Prefixes each non-header line with a 5-character right-aligned number.
/// Header lines (@Begin, @Languages, etc.) are not numbered.
pub fn add_line_numbers(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + text.lines().count() * 6);
    let mut line_num: u64 = 0;
    for line in text.lines() {
        if line.starts_with('@') {
            out.push_str(line);
        } else {
            line_num += 1;
            out.push_str(&format!("{line_num:>5} {line}"));
        }
        out.push('\n');
    }
    out
}

/// Remove line number prefixes from CHAT text lines.
///
/// Strips the first 6 characters (5-digit number + space) from non-header lines.
pub fn remove_line_numbers(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        if line.starts_with('@') {
            out.push_str(line);
        } else if line.len() > 6 {
            out.push_str(&line[6..]);
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

/// Custom run function for LINES that applies line numbering to serialized output.
pub fn run_lines(
    config: &LinesConfig,
    input: &std::path::Path,
    output: Option<&std::path::Path>,
) -> Result<(), TransformError> {
    use std::io::Write;

    let content = std::fs::read_to_string(input)?;

    let result = if config.remove {
        remove_line_numbers(&content)
    } else {
        // Parse and re-serialize for canonical formatting, then add numbers
        let chat_file = talkbank_transform::parse_and_validate(
            &content,
            talkbank_model::ParseValidateOptions::default(),
        )
        .map_err(|e| TransformError::Parse(e.to_string()))?;
        let serialized = talkbank_model::WriteChat::to_chat_string(&chat_file);
        add_line_numbers(&serialized)
    };

    if let Some(output_path) = output {
        std::fs::write(output_path, &result)?;
    } else {
        std::io::stdout().write_all(result.as_bytes())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_numbers() {
        let input = "@Begin\n*CHI:\thello .\n%mor:\tn|hello .\n@End\n";
        let result = add_line_numbers(input);
        assert!(result.contains("@Begin"));
        assert!(result.contains("    1 *CHI:"));
        assert!(result.contains("    2 %mor:"));
        assert!(result.contains("@End"));
    }

    #[test]
    fn remove_numbers() {
        let input = "@Begin\n    1 *CHI:\thello .\n    2 %mor:\tn|hello .\n@End\n";
        let result = remove_line_numbers(input);
        assert!(result.contains("*CHI:\thello ."));
        assert!(result.contains("%mor:\tn|hello ."));
    }
}
