//! Cantonese text normalization for ASR post-processing.
//!
//! Ported from Python `python/batchalign/inference/languages/cantonese/_common.py`. Applies:
//! 1. Simplified → HK Traditional Chinese conversion via embedded OpenCC rules
//! 2. Domain-specific replacement table (31 entries) for Cantonese-specific
//!    character corrections

use std::sync::LazyLock;

use ferrous_opencc::{OpenCC, config::BuiltinConfig};

// ---------------------------------------------------------------------------
// Replacement table
// ---------------------------------------------------------------------------

/// Domain-specific Cantonese replacements applied AFTER zh-HK conversion.
///
/// Multi-character entries come first to prevent partial matches (e.g.,
/// "系" → "係" firing before "聯係" → "聯繫").
///
/// Ordered longest-first so that `replace_all` via sequential scan handles
/// overlapping patterns correctly.
const REPLACEMENTS: &[(&str, &str)] = &[
    // Multi-character (longest first)
    ("聯繫", "聯繫"),
    ("聯係", "聯繫"),
    ("系啊", "係啊"),
    ("真系", "真係"),
    ("唔系", "唔係"),
    ("中意", "鍾意"),
    ("遊水", "游水"),
    ("羣組", "群組"),
    ("古仔", "故仔"),
    ("較剪", "鉸剪"),
    ("衝涼", "沖涼"),
    ("分鍾", "分鐘"),
    ("重復", "重複"),
    // Single-character
    ("系", "係"),
    ("繫", "係"),
    ("呀", "啊"),
    ("噶", "㗎"),
    ("咧", "呢"),
    ("嗬", "喎"),
    ("只", "隻"),
    ("咯", "囉"),
    ("嚇", "吓"),
    ("啫", "咋"),
    ("哇", "嘩"),
    ("着", "著"),
    ("嘞", "喇"),
    ("啵", "噃"),
    ("甕", "㧬"),
    ("牀", "床"),
    ("松", "鬆"),
    ("吵", "嘈"),
];

/// CJK punctuation and whitespace to strip during char tokenization.
///
/// Matches: fullwidth space, ideographic comma, ideographic period,
/// fullwidth comma, fullwidth exclamation, fullwidth question mark,
/// left/right corner brackets, fullwidth colon, fullwidth semicolon,
/// and ASCII whitespace.
fn is_cjk_punct_or_space(c: char) -> bool {
    matches!(
        c,
        '\u{3000}'  // ideographic space
        | '\u{3001}' // ideographic comma
        | '\u{3002}' // ideographic period
        | '\u{FF0C}' // fullwidth comma
        | '\u{FF01}' // fullwidth exclamation
        | '\u{FF1F}' // fullwidth question mark
        | '\u{300C}' // left corner bracket
        | '\u{300D}' // right corner bracket
        | '\u{FF1A}' // fullwidth colon
        | '\u{FF1B}' // fullwidth semicolon
    ) || c.is_ascii_whitespace()
}

// ---------------------------------------------------------------------------
// Aho-Corasick replacement engine
// ---------------------------------------------------------------------------

#[allow(clippy::expect_used)]
static HK_OPENCC: LazyLock<OpenCC> = LazyLock::new(|| {
    OpenCC::from_config(BuiltinConfig::S2hk)
        .expect("embedded S2hk conversion tables should be available")
});

/// Pre-built Aho-Corasick automaton for the replacement table.
///
/// Uses leftmost-longest matching to handle overlapping patterns correctly
/// (multi-char entries like "聯係" match before single-char "系").
// Data is compile-time-constant: patterns are static string literals defined above.
#[allow(clippy::expect_used)]
static REPLACER: LazyLock<aho_corasick::AhoCorasick> = LazyLock::new(|| {
    let patterns: Vec<&str> = REPLACEMENTS.iter().map(|(from, _)| *from).collect();
    aho_corasick::AhoCorasick::builder()
        .match_kind(aho_corasick::MatchKind::LeftmostLongest)
        .build(&patterns)
        .expect("cantonese replacement patterns are valid")
});

/// Apply the domain-specific replacement table using Aho-Corasick.
fn apply_replacements(text: &str) -> String {
    let replacements: Vec<&str> = REPLACEMENTS.iter().map(|(_, to)| *to).collect();
    REPLACER.replace_all(text, &replacements)
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Normalize Cantonese text: simplified→HK traditional + domain replacements.
///
/// This is the Rust equivalent of Python's `normalize_cantonese_text()`.
pub fn normalize_cantonese(text: &str) -> String {
    let converted = HK_OPENCC.convert(text);
    apply_replacements(&converted)
}

/// Normalize text and split into per-character tokens, stripping CJK punctuation.
///
/// This is the Rust equivalent of Python's `normalize_cantonese_char_tokens()`.
/// Used by FunASR Cantonese to align per-character timestamps.
pub fn cantonese_char_tokens(text: &str) -> Vec<String> {
    let normalized = normalize_cantonese(text);
    normalized
        .chars()
        .filter(|c| !is_cjk_punct_or_space(*c))
        .map(|c| c.to_string())
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_char_replacement() {
        assert_eq!(normalize_cantonese("系"), "係");
        assert_eq!(normalize_cantonese("呀"), "啊");
        assert_eq!(normalize_cantonese("松"), "鬆");
        assert_eq!(normalize_cantonese("吵"), "嘈");
    }

    #[test]
    fn test_multi_char_replacement() {
        assert_eq!(normalize_cantonese("真系"), "真係");
        assert_eq!(normalize_cantonese("中意"), "鍾意");
        assert_eq!(normalize_cantonese("較剪"), "鉸剪");
    }

    #[test]
    fn test_multi_char_priority_over_single() {
        // "聯係" should become "聯繫", not "聯" + "係"
        assert_eq!(normalize_cantonese("聯係"), "聯繫");
        // "系啊" should match as a unit, not "系" + "啊"
        assert_eq!(normalize_cantonese("系啊"), "係啊");
    }

    #[test]
    fn test_opencc_simplified_to_hk() {
        // Embedded OpenCC rules handle standard simplified→HK traditional conversion.
        assert_eq!(normalize_cantonese("联系"), "聯繫");
    }

    #[test]
    fn test_full_sentence() {
        assert_eq!(normalize_cantonese("你真系好吵呀"), "你真係好嘈啊");
    }

    #[test]
    fn test_idempotent_on_hk_text() {
        assert_eq!(normalize_cantonese("你好"), "你好");
    }

    #[test]
    fn test_char_tokens_basic() {
        let tokens = cantonese_char_tokens("真系呀，");
        assert_eq!(tokens, vec!["真", "係", "啊"]);
    }

    #[test]
    fn test_char_tokens_strips_all_cjk_punct() {
        let tokens = cantonese_char_tokens("「你好」！");
        assert_eq!(tokens, vec!["你", "好"]);
    }

    #[test]
    fn test_char_tokens_empty() {
        let tokens = cantonese_char_tokens("");
        assert!(tokens.is_empty());
    }

    #[test]
    fn test_char_tokens_only_punct() {
        let tokens = cantonese_char_tokens("，。！？");
        assert!(tokens.is_empty());
    }

    #[test]
    fn test_replacement_table_after_opencc() {
        // "松" after S2HK conversion should still become "鬆" via replacement table.
        assert_eq!(normalize_cantonese("松"), "鬆");
    }
}
