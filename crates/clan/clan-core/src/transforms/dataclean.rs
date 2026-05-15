//! DATACLEAN -- fix common CHAT formatting errors.
//!
//! Reimplements CLAN's DataCleanUp command, which fixes spacing and formatting
//! issues in CHAT files. Because these are text-level formatting concerns that
//! operate below the AST level, the [`DatacleanCommand`] struct is an AST no-op;
//! the actual logic lives in [`clean_chat_text()`] and the end-to-end
//! [`run_dataclean()`] function.
//!
//! # Fixes applied
//!
//! - Missing space before `[` brackets
//! - Missing space after `]` brackets
//! - Tab characters inside lines (converted to spaces)
//! - Bare `...` without `+` prefix (converted to `+...`)
//! - `#long` converted to `##`
//! - Header lines (`@`-prefixed) are left untouched
//!
//! # Differences from CLAN
//!
//! - The AST transform is a no-op; fixes are applied to serialized text via
//!   `clean_chat_text()` after the standard parse → serialize round-trip.
//! - This hybrid approach (parse → serialize → text fix) ensures structural
//!   integrity from the parse step while still handling sub-AST formatting
//!   concerns that CLAN fixes with raw text manipulation.

use talkbank_model::ChatFile;

use crate::framework::{TransformCommand, TransformError};

/// Configuration for the DATACLEAN command.
pub struct DatacleanConfig {
    /// Only fix spacing/bracket issues (default: fix everything).
    pub spacing_only: bool,
}

/// DATACLEAN transform (AST no-op — use `run_dataclean` for the actual logic).
pub struct DatacleanCommand;

impl TransformCommand for DatacleanCommand {
    type Config = DatacleanConfig;

    /// AST no-op. Use [`run_dataclean()`] for the actual text-level fixes.
    fn transform(&self, _file: &mut ChatFile) -> Result<(), TransformError> {
        // DataClean operates on serialized text. The AST transform is a no-op.
        // Use `run_dataclean` for the actual text-level fixes.
        Ok(())
    }
}

/// Apply dataclean fixes to serialized CHAT text.
pub fn clean_chat_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut changes = 0u64;

    for line in text.lines() {
        let cleaned = clean_line(line, &mut changes);
        out.push_str(&cleaned);
        out.push('\n');
    }

    if changes > 0 {
        tracing::info!("{changes} formatting fixes applied");
    }
    out
}

/// Clean a single line of CHAT text.
fn clean_line(line: &str, changes: &mut u64) -> String {
    if line.starts_with('@') {
        return line.to_owned();
    }

    let mut result = String::with_capacity(line.len() + 16);
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut in_bracket = false;

    while i < len {
        let c = chars[i];

        // Track bracket nesting (before spacing fixes so state is current)
        if c == '[' {
            in_bracket = true;
        } else if c == ']' {
            in_bracket = false;
        }

        // Bracket spacing fixes always apply, even on bracket characters themselves

        // Fix missing space before '['
        if c == '[' && i > 0 && !chars[i - 1].is_whitespace() && chars[i - 1] != '[' {
            result.push(' ');
            *changes += 1;
        }

        // Fix missing space after ']'
        if i > 0 && chars[i - 1] == ']' && !c.is_whitespace() && c != ']' && c != '[' && c != '\n' {
            result.push(' ');
            *changes += 1;
        }

        // Skip other fixes for content inside brackets
        if in_bracket {
            result.push(c);
            i += 1;
            continue;
        }

        // Fix tabs inside lines (after the speaker prefix)
        if c == '\t' && !result.is_empty() && !result.ends_with(':') {
            result.push(' ');
            *changes += 1;
            i += 1;
            continue;
        }

        // Fix "..." → "+..." (when not preceded by '+')
        if c == '.'
            && i + 2 < len
            && chars[i + 1] == '.'
            && chars[i + 2] == '.'
            && (i == 0 || chars[i - 1] != '+')
        {
            if i > 0 && !chars[i - 1].is_whitespace() {
                result.push(' ');
            }
            result.push_str("+...");
            *changes += 1;
            i += 3;
            continue;
        }

        // Fix "#long" → "##"
        if c == '#'
            && i + 4 < len
            && chars[i + 1] == 'l'
            && chars[i + 2] == 'o'
            && chars[i + 3] == 'n'
            && chars[i + 4] == 'g'
            && (i + 5 >= len || chars[i + 5].is_whitespace())
        {
            result.push_str("##");
            *changes += 1;
            i += 5;
            continue;
        }

        result.push(c);
        i += 1;
    }

    result
}

/// Custom run function for DATACLEAN that applies text-level fixes.
pub fn run_dataclean(
    input: &std::path::Path,
    output: Option<&std::path::Path>,
) -> Result<(), TransformError> {
    use std::io::Write;

    let content = std::fs::read_to_string(input)?;

    // Parse → serialize to canonical form → apply text fixes
    let chat_file = talkbank_transform::parse_and_validate(
        &content,
        talkbank_model::ParseValidateOptions::default(),
    )
    .map_err(|e| TransformError::Parse(e.to_string()))?;
    let serialized = talkbank_model::WriteChat::to_chat_string(&chat_file);
    let cleaned = clean_chat_text(&serialized);

    if let Some(output_path) = output {
        std::fs::write(output_path, &cleaned)?;
    } else {
        std::io::stdout().write_all(cleaned.as_bytes())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fix_bracket_spacing() {
        let mut changes = 0;
        let result = clean_line("*CHI:\thello[+ test] .", &mut changes);
        assert!(result.contains("hello [+ test]"));
        assert!(changes > 0);
    }

    #[test]
    fn fix_ellipsis() {
        let mut changes = 0;
        let result = clean_line("*CHI:\thello... .", &mut changes);
        assert!(result.contains("+..."));
    }

    #[test]
    fn fix_hash_long() {
        let mut changes = 0;
        let result = clean_line("*CHI:\thello #long .", &mut changes);
        assert!(result.contains("##"));
        assert!(!result.contains("#long"));
    }

    #[test]
    fn headers_unchanged() {
        let mut changes = 0;
        let result = clean_line("@Begin", &mut changes);
        assert_eq!(result, "@Begin");
        assert_eq!(changes, 0);
    }
}
