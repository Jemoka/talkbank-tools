//! Public token parsing API for coarsened grammar tokens.
//!
//! When the tree-sitter grammar coarsens a structured rule into an atomic `token(...)`,
//! the CST node becomes an opaque string. This module provides the canonical parse
//! functions to extract typed model values from that string.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Words>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#CA_Overlaps>
//! - <https://talkbank.org/0info/manuals/CHAT.html#SecondLanguage_Marker_Single>
//!
//! These functions are used by:
//! - The tree-sitter parser's CST-to-model conversion
//! - The LSP and VS Code extension for hover/diagnostic info
//! - External tools (Python bindings, etc.) that read the CST directly
//!
//! # Token catalog
//!
//! | Token | Example | Function |
//! |-------|---------|----------|
//! | `langcode` | `[- eng]` | [`parse_langcode_token`] |
//! | `explanation_annotation` | `[= laughing]` | [`parse_explanation_token`] |
//! | `para_annotation` | `[=! whispers]` | [`parse_para_token`] |
//! | `alt_annotation` | `[=? word]` | [`parse_alt_token`] |
//! | `percent_annotation` | `[% comment]` | [`parse_percent_token`] |
//! | `duration_annotation` | `[# 2.5]` | [`parse_duration_token`] |
//! | `error_marker_annotation` | `[*]`, `[* s:r]` | [`parse_error_marker_token`] |
//! | `indexed_overlap_precedes` | `[<]`, `[<2]` | [`parse_overlap_precedes_token`] |
//! | `indexed_overlap_follows` | `[>]`, `[>2]` | [`parse_overlap_follows_token`] |
//! | `age_format` | `2;05.24` | [`parse_age_format_token`] |

use talkbank_model::model::{
    AgeValue, ContentAnnotation, LanguageCode, OverlapMarkerIndex, ScopedAlternative, ScopedError,
    ScopedExplanation, ScopedOverlapBegin, ScopedOverlapEnd, ScopedParalinguistic,
    ScopedPercentComment,
};

// ---------------------------------------------------------------------------
// Language code: [- eng]
// ---------------------------------------------------------------------------

/// Parse an atomic `langcode` token into a [`LanguageCode`].
///
/// Input: the full token text, e.g. `"[- eng]"`.
/// Returns `None` if the format doesn't match `[- <code>]`.
pub fn parse_langcode_token(token_text: &str) -> Option<LanguageCode> {
    let code = token_text.strip_prefix("[- ")?.strip_suffix(']')?;
    if code.is_empty() {
        return None;
    }
    Some(LanguageCode::new(code))
}

// ---------------------------------------------------------------------------
// Text-bearing annotations: [= text], [=! text], [=? text], [% text], [# time]
// ---------------------------------------------------------------------------

/// Strip a known prefix and closing `]` from a token, returning the inner text.
/// Trims trailing whitespace for leniency.
fn strip_annotation(token_text: &str, prefix: &str) -> Option<smol_str::SmolStr> {
    let inner = token_text.strip_prefix(prefix)?.strip_suffix(']')?;
    let trimmed = inner.trim_end();
    if trimmed.is_empty() {
        return None;
    }
    Some(smol_str::SmolStr::from(trimmed))
}

/// Parse an atomic `explanation_annotation` token `[= text]`.
///
/// Returns `None` if the format doesn't match.
pub fn parse_explanation_token(token_text: &str) -> Option<ContentAnnotation> {
    strip_annotation(token_text, "[= ")
        .map(|text| ContentAnnotation::Explanation(ScopedExplanation { text }))
}

/// Parse an atomic `para_annotation` token `[=! text]`.
///
/// Returns `None` if the format doesn't match.
pub fn parse_para_token(token_text: &str) -> Option<ContentAnnotation> {
    strip_annotation(token_text, "[=! ")
        .map(|text| ContentAnnotation::Paralinguistic(ScopedParalinguistic { text }))
}

/// Parse an atomic `alt_annotation` token `[=? text]`.
///
/// Returns `None` if the format doesn't match.
pub fn parse_alt_token(token_text: &str) -> Option<ContentAnnotation> {
    strip_annotation(token_text, "[=? ")
        .map(|text| ContentAnnotation::Alternative(ScopedAlternative { text }))
}

/// Parse an atomic `percent_annotation` token `[% text]`.
///
/// Returns `None` if the format doesn't match.
pub fn parse_percent_token(token_text: &str) -> Option<ContentAnnotation> {
    strip_annotation(token_text, "[% ")
        .map(|text| ContentAnnotation::PercentComment(ScopedPercentComment { text }))
}

// ---------------------------------------------------------------------------
// Error marker: [*] or [* code]
// ---------------------------------------------------------------------------

/// Parse an atomic `error_marker_annotation` token `[*]` or `[* code]`.
///
/// Returns `ContentAnnotation::Error` with an optional error code string.
/// Returns `None` only if the token doesn't match the `[*...]` format at all.
pub fn parse_error_marker_token(token_text: &str) -> Option<ContentAnnotation> {
    let inner = token_text.strip_prefix('[')?.strip_suffix(']')?;
    let after_star = inner.strip_prefix('*')?;

    let code = if after_star.is_empty() {
        // [*] — no code
        None
    } else {
        // [* code] — strip leading space
        let trimmed = after_star.strip_prefix(' ').unwrap_or(after_star).trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(smol_str::SmolStr::from(trimmed))
        }
    };

    Some(ContentAnnotation::Error(ScopedError { code }))
}

// ---------------------------------------------------------------------------
// Overlap annotations: [<], [<N], [>], [>N]
// ---------------------------------------------------------------------------

/// Extract optional overlap index (1-9) from inside the brackets.
fn extract_overlap_index(inner: &str) -> Option<OverlapMarkerIndex> {
    let trimmed = inner.trim();
    if trimmed.len() == 1 {
        let ch = trimmed.as_bytes()[0];
        if ch.is_ascii_digit() && ch != b'0' {
            return Some(OverlapMarkerIndex::new(ch - b'0'));
        }
    }
    None
}

/// Parse an atomic `indexed_overlap_precedes` token `[<]` or `[<N]`.
///
/// Returns `ContentAnnotation::OverlapBegin` with optional index.
/// Returns `None` if the format doesn't match.
pub fn parse_overlap_precedes_token(token_text: &str) -> Option<ContentAnnotation> {
    let inner = token_text.strip_prefix("[<")?.strip_suffix(']')?;
    let index = extract_overlap_index(inner);
    Some(ContentAnnotation::OverlapBegin(ScopedOverlapBegin {
        index,
    }))
}

/// Parse an atomic `indexed_overlap_follows` token `[>]` or `[>N]`.
///
/// Returns `ContentAnnotation::OverlapEnd` with optional index.
/// Returns `None` if the format doesn't match.
pub fn parse_overlap_follows_token(token_text: &str) -> Option<ContentAnnotation> {
    let inner = token_text.strip_prefix("[>")?.strip_suffix(']')?;
    let index = extract_overlap_index(inner);
    Some(ContentAnnotation::OverlapEnd(ScopedOverlapEnd { index }))
}

// ---------------------------------------------------------------------------
// Age format: years;months.days
// ---------------------------------------------------------------------------

/// Parse an atomic `age_format` token `years;months.days`.
///
/// Accepts formats: `"2;05.24"`, `"1;08."`, `"3;06"`, `"10;0"`.
/// Returns `None` if the format doesn't match `DIGITS;DIGITS[.DIGITS?]`.
pub fn parse_age_format_token(token_text: &str) -> Option<AgeValue> {
    let (years, rest) = token_text.split_once(';')?;
    if years.is_empty() || !years.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    // rest is "months" or "months." or "months.days"
    let months_days = rest;
    if months_days.is_empty() {
        return None;
    }
    if let Some((months, days)) = months_days.split_once('.') {
        if months.is_empty()
            || !months.bytes().all(|b| b.is_ascii_digit())
            || !days.bytes().all(|b| b.is_ascii_digit())
        {
            return None;
        }
    } else if !months_days.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(AgeValue::new(token_text))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests langcode basic.
    #[test]
    fn langcode_basic() {
        let lc = parse_langcode_token("[- eng]").unwrap();
        assert_eq!(lc.to_string(), "eng");
    }

    /// Tests langcode missing code.
    #[test]
    fn langcode_missing_code() {
        assert!(parse_langcode_token("[- ]").is_none());
    }

    /// Tests explanation.
    #[test]
    fn explanation() {
        let ann = parse_explanation_token("[= laughing]").unwrap();
        assert!(matches!(ann, ContentAnnotation::Explanation(e) if e.text == "laughing"));
    }

    /// Tests para.
    #[test]
    fn para() {
        let ann = parse_para_token("[=! whispers]").unwrap();
        assert!(matches!(ann, ContentAnnotation::Paralinguistic(p) if p.text == "whispers"));
    }

    /// Tests alt.
    #[test]
    fn alt() {
        let ann = parse_alt_token("[=? word]").unwrap();
        assert!(matches!(ann, ContentAnnotation::Alternative(a) if a.text == "word"));
    }

    /// Tests percent.
    #[test]
    fn percent() {
        let ann = parse_percent_token("[% some comment]").unwrap();
        assert!(matches!(ann, ContentAnnotation::PercentComment(p) if p.text == "some comment"));
    }

    /// Tests error marker no code.
    #[test]
    fn error_marker_no_code() {
        let ann = parse_error_marker_token("[*]").unwrap();
        assert!(matches!(ann, ContentAnnotation::Error(e) if e.code.is_none()));
    }

    /// Tests error marker with code.
    #[test]
    fn error_marker_with_code() {
        let ann = parse_error_marker_token("[* s:r]").unwrap();
        assert!(matches!(ann, ContentAnnotation::Error(e) if e.code.as_deref() == Some("s:r")));
    }

    /// Tests overlap precedes no index.
    #[test]
    fn overlap_precedes_no_index() {
        let ann = parse_overlap_precedes_token("[<]").unwrap();
        assert!(matches!(ann, ContentAnnotation::OverlapBegin(o) if o.index.is_none()));
    }

    /// Tests overlap precedes with index.
    #[test]
    fn overlap_precedes_with_index() {
        let ann = parse_overlap_precedes_token("[<2]").unwrap();
        assert!(
            matches!(ann, ContentAnnotation::OverlapBegin(o) if o.index.map(|i| i.0) == Some(2))
        );
    }

    /// Tests overlap follows no index.
    #[test]
    fn overlap_follows_no_index() {
        let ann = parse_overlap_follows_token("[>]").unwrap();
        assert!(matches!(ann, ContentAnnotation::OverlapEnd(o) if o.index.is_none()));
    }

    /// Tests overlap follows with index.
    #[test]
    fn overlap_follows_with_index() {
        let ann = parse_overlap_follows_token("[>3]").unwrap();
        assert!(matches!(ann, ContentAnnotation::OverlapEnd(o) if o.index.map(|i| i.0) == Some(3)));
    }

    /// Tests overlap with spaces.
    #[test]
    fn overlap_with_spaces() {
        // [< ] — space but no index
        let ann = parse_overlap_precedes_token("[< ]").unwrap();
        assert!(matches!(ann, ContentAnnotation::OverlapBegin(o) if o.index.is_none()));
    }

    /// Tests age format full.
    #[test]
    fn age_format_full() {
        let age = parse_age_format_token("2;05.24").unwrap();
        assert_eq!(age.to_string(), "2;05.24");
    }

    /// Tests age format no days.
    #[test]
    fn age_format_no_days() {
        let age = parse_age_format_token("3;06").unwrap();
        assert_eq!(age.to_string(), "3;06");
    }

    /// Tests age format trailing dot.
    #[test]
    fn age_format_trailing_dot() {
        let age = parse_age_format_token("1;08.").unwrap();
        assert_eq!(age.to_string(), "1;08.");
    }

    /// Tests age format invalid.
    #[test]
    fn age_format_invalid() {
        assert!(parse_age_format_token("abc").is_none());
        assert!(parse_age_format_token(";05").is_none());
        assert!(parse_age_format_token("2;").is_none());
    }
}
