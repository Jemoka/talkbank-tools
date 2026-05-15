//! LONGTIER -- remove line continuation wrapping.
//!
//! Reimplements CLAN's `longtier` command, which removes line continuation
//! markers (newline + tab) from all tiers so each tier occupies exactly one
//! line in the output file.
//!
//! In CHAT format, tiers longer than ~80 characters are conventionally
//! wrapped with a newline followed by a tab. This command folds those
//! continuation lines back into their parent tier.
//!
//! Because this is a text-level formatting concern (below the AST level),
//! the actual logic lives in [`fold_continuation_lines()`] and the end-to-end
//! [`run_longtier()`] function, similar to DATACLEAN and LINES.
//!
//! # Differences from CLAN
//!
//! - Operates on raw text rather than partial parsing, making it robust
//!   against malformed files that might not parse cleanly.
//! - Normalizes all newlines to `\n` (handles `\r\n` and `\r`).
//! - Multiple leading tabs on continuation lines are all consumed.

use std::fs;
use std::io::{self, Write as IoWrite};
use std::path::Path;

use crate::framework::TransformError;

/// Fold continuation lines in CHAT content.
///
/// A continuation line is any line that starts with one or more tab characters.
/// The content after the tabs is appended to the previous logical line with a
/// single space. Newlines are normalized to `\n`.
pub fn fold_continuation_lines(input: &str) -> FoldResult {
    let mut output = String::with_capacity(input.len());
    let mut continuation_count = 0;
    let mut chars = input.chars().peekable();
    let mut at_line_start = true;
    let mut prev_was_newline = false;

    while let Some(c) = chars.next() {
        match c {
            '\r' | '\n' => {
                if !prev_was_newline {
                    prev_was_newline = true;
                }
                // Consume additional newline chars
                while chars.peek() == Some(&'\r') || chars.peek() == Some(&'\n') {
                    chars.next();
                }
                at_line_start = true;
            }
            '\t' if at_line_start => {
                // Continuation line — skip all leading tabs
                while chars.peek() == Some(&'\t') {
                    chars.next();
                }
                // Join with previous line via space
                if !output.is_empty() && !output.ends_with('\n') {
                    output.push(' ');
                }
                continuation_count += 1;
                at_line_start = false;
                prev_was_newline = false;
            }
            _ => {
                if prev_was_newline {
                    output.push('\n');
                    prev_was_newline = false;
                }
                output.push(c);
                at_line_start = false;
            }
        }
    }

    // Trailing newline
    if prev_was_newline && !output.is_empty() {
        output.push('\n');
    }

    FoldResult {
        content: output,
        continuation_count,
    }
}

/// Result of folding continuation lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoldResult {
    /// The folded content with continuations merged.
    pub content: String,
    /// Number of continuation lines that were folded.
    pub continuation_count: usize,
}

impl FoldResult {
    /// Returns true if any continuations were folded.
    pub fn changed(&self) -> bool {
        self.continuation_count > 0
    }
}

/// Run LONGTIER on a file: fold continuation lines and write output.
pub fn run_longtier(input: &Path, output: Option<&Path>) -> Result<(), TransformError> {
    let content = fs::read_to_string(input).map_err(TransformError::Io)?;
    let result = fold_continuation_lines(&content);

    if let Some(output_path) = output {
        fs::write(output_path, &result.content).map_err(TransformError::Io)?;
    } else {
        io::stdout()
            .write_all(result.content.as_bytes())
            .map_err(TransformError::Io)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_continuations() {
        let input = "@UTF8\n@Begin\n*CHI:\thello .\n@End\n";
        let result = fold_continuation_lines(input);
        assert_eq!(result.continuation_count, 0);
        assert_eq!(result.content, input);
    }

    #[test]
    fn single_continuation() {
        let input = "*CHI:\they man\n\twhat is this ?\n";
        let result = fold_continuation_lines(input);
        assert_eq!(result.continuation_count, 1);
        assert_eq!(result.content, "*CHI:\they man what is this ?\n");
    }

    #[test]
    fn multiple_continuations() {
        let input = "*CHI:\they man\n\twhat in the world\n\tis this ?\n";
        let result = fold_continuation_lines(input);
        assert_eq!(result.continuation_count, 2);
        assert_eq!(
            result.content,
            "*CHI:\they man what in the world is this ?\n"
        );
    }

    #[test]
    fn multiple_utterances() {
        let input = "*CHI:\they\n\tman .\n*MOT:\thi\n\tthere .\n";
        let result = fold_continuation_lines(input);
        assert_eq!(result.continuation_count, 2);
        assert_eq!(result.content, "*CHI:\they man .\n*MOT:\thi there .\n");
    }

    #[test]
    fn multi_tab_continuation() {
        let input = "*CHI:\they man\n\t\twhat is this ?\n";
        let result = fold_continuation_lines(input);
        assert_eq!(result.continuation_count, 1);
        assert_eq!(result.content, "*CHI:\they man what is this ?\n");
    }

    #[test]
    fn crlf_newlines() {
        let input = "*CHI:\they man\r\n\twhat is this ?\r\n";
        let result = fold_continuation_lines(input);
        assert_eq!(result.continuation_count, 1);
        assert_eq!(result.content, "*CHI:\they man what is this ?\n");
    }

    #[test]
    fn unicode_preservation() {
        let input = "*CHI:\t你好\n\t世界 .\n";
        let result = fold_continuation_lines(input);
        assert_eq!(result.continuation_count, 1);
        assert_eq!(result.content, "*CHI:\t你好 世界 .\n");
    }

    #[test]
    fn realistic_chat_file() {
        let input = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Child, MOT Mother
*CHI:\they man
\twhat is this ?
*MOT:\tnothing !
@End
";
        let result = fold_continuation_lines(input);
        assert_eq!(result.continuation_count, 1);
        let expected = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Child, MOT Mother
*CHI:\they man what is this ?
*MOT:\tnothing !
@End
";
        assert_eq!(result.content, expected);
    }
}
