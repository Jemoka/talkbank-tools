//! CLAN-adjusted error location resolution.
//!
//! CLAN hides certain header lines (@UTF8, @PID, @Font, @ColorWords, @Window)
//! from its editor display and line numbering. When sending an error location
//! to CLAN, the line number must be adjusted to account for these hidden lines.
//!
//! This module provides [`resolve_clan_location`], the single function that
//! both the TUI and desktop app use to convert a `ParseError`'s location into
//! CLAN-compatible coordinates.

use super::source_location::SourceLocation;

/// CLAN-adjusted line and column for sending to the CLAN editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClanLocation {
    /// 1-indexed line number in CLAN's display (hidden headers subtracted).
    pub line: usize,
    /// 1-indexed column number.
    pub column: usize,
}

/// Error returned when the error is on a line that CLAN hides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClanHiddenLineError {
    /// The original 1-indexed line number in the source file.
    pub source_line: usize,
}

impl std::fmt::Display for ClanHiddenLineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Error is on line {} which is a CLAN hidden header \
             (@UTF8/@PID/@Font/@ColorWords/@Window) — CLAN does not display this line",
            self.source_line
        )
    }
}

impl std::error::Error for ClanHiddenLineError {}

/// Resolve a `ParseError`'s location into CLAN-compatible coordinates.
///
/// Handles the full pipeline:
/// 1. If `location.line`/`column` are populated, use them directly
/// 2. Otherwise, compute from `location.span.start` byte offset and the source text
/// 3. Subtract hidden CLAN headers from the line number
///
/// Returns `Err` if the error falls on a hidden header line.
pub fn resolve_clan_location(
    location: &SourceLocation,
    source: &str,
) -> Result<ClanLocation, ClanHiddenLineError> {
    let (line, column) = match (location.line, location.column) {
        (Some(line), Some(column)) if line >= 1 && column >= 1 => (line, column),
        _ => SourceLocation::calculate_line_column(location.span.start as usize, source),
    };

    let hidden = count_clan_hidden_lines(source, line);
    let clan_line = line as isize - hidden as isize;

    if clan_line < 1 {
        return Err(ClanHiddenLineError { source_line: line });
    }

    Ok(ClanLocation {
        line: clan_line as usize,
        column,
    })
}

/// Header prefixes that CLAN hides from its editor display and line numbering.
const CLAN_HIDDEN_PREFIXES: &[&str] = &["@UTF8", "@PID", "@Font", "@ColorWords", "@Window"];

/// Count header lines before `up_to_line` (1-indexed) that CLAN hides.
fn count_clan_hidden_lines(source: &str, up_to_line: usize) -> usize {
    source
        .lines()
        .take(up_to_line)
        .filter(|line| {
            CLAN_HIDDEN_PREFIXES
                .iter()
                .any(|prefix| line.starts_with(prefix))
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Span;

    fn loc(start: u32, end: u32, line: Option<usize>, column: Option<usize>) -> SourceLocation {
        SourceLocation {
            span: Span::new(start, end),
            line,
            column,
        }
    }

    #[test]
    fn explicit_line_col_used_when_present() {
        let source = "@UTF8\n@Begin\n*CHI:\thello .\n@End\n";
        let result = resolve_clan_location(&loc(13, 27, Some(3), Some(1)), source).unwrap();
        // Line 3 (*CHI:), 1 hidden header (@UTF8) → CLAN line 2
        assert_eq!(result, ClanLocation { line: 2, column: 1 });
    }

    #[test]
    fn byte_offset_used_when_line_col_missing() {
        let source = "@UTF8\n@Begin\n*CHI:\thello .\n@End\n";
        // byte 13 = start of "*CHI:" on line 3
        let result = resolve_clan_location(&loc(13, 27, None, None), source).unwrap();
        assert_eq!(result, ClanLocation { line: 2, column: 1 });
    }

    #[test]
    fn multiple_hidden_headers() {
        let source = "@UTF8\n@PID:\t123\n@Font:\tWin\n@Begin\n*CHI:\thello .\n@End\n";
        // Line 5 (*CHI:), 3 hidden headers (@UTF8, @PID, @Font) → CLAN line 2
        let result = resolve_clan_location(&loc(0, 1, Some(5), Some(1)), source).unwrap();
        assert_eq!(result, ClanLocation { line: 2, column: 1 });
    }

    #[test]
    fn error_on_hidden_line_is_rejected() {
        let source = "@UTF8\n@Begin\n";
        let result = resolve_clan_location(&loc(0, 5, Some(1), Some(1)), source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.source_line, 1);
        assert!(err.to_string().contains("hidden"));
    }

    #[test]
    fn no_hidden_headers() {
        let source = "@Begin\n*CHI:\thello .\n@End\n";
        let result = resolve_clan_location(&loc(0, 1, Some(2), Some(1)), source).unwrap();
        assert_eq!(result, ClanLocation { line: 2, column: 1 });
    }

    #[test]
    fn window_header_is_hidden() {
        let source = "@UTF8\n@Window:\t100,0\n@Begin\n*CHI:\thello .\n@End\n";
        // Line 4 (*CHI:), 2 hidden (@UTF8, @Window) → CLAN line 2
        let result = resolve_clan_location(&loc(0, 1, Some(4), Some(1)), source).unwrap();
        assert_eq!(result, ClanLocation { line: 2, column: 1 });
    }

    #[test]
    fn colorwords_header_is_hidden() {
        let source = "@UTF8\n@ColorWords:\t$BLU\n@Begin\n*CHI:\thello .\n@End\n";
        // Line 4, 2 hidden → CLAN line 2
        let result = resolve_clan_location(&loc(0, 1, Some(4), Some(1)), source).unwrap();
        assert_eq!(result, ClanLocation { line: 2, column: 1 });
    }
}
