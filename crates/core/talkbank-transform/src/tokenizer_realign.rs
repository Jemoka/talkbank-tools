//! Tokenizer realignment: merge Stanza tokens back to original CHAT words.
//!
//! When Stanza's neural tokenizer runs, it may re-split compound words
//! (e.g. "ice-cream" → `["ice", "-", "cream"]`). This module merges those
//! spurious splits back, preserving the 1-to-1 mapping between CHAT words
//! and Stanza tokens.
//!
//! # Architecture
//!
//! This module is the shared implementation used by both:
//! - The PyO3 bridge (`batchalign-core`) — called from `align_tokens` pyfunction
//! - The standalone Rust server (`batchalign-server`) — called directly
//!
//! The PyO3 crate provides only a thin wrapper that converts `Vec<PatchedToken>`
//! to Python objects (`str` or `(str, bool)` tuples).
//!
//! # MWT Hint Convention
//!
//! Stanza's `tokenize_postprocessor` uses a tuple convention:
//!
//! - `(text, True)`  — MWT: let the MWT processor expand (e.g. "don't" → do + n't)
//! - `(text, False)` — NOT an MWT: suppress expansion (e.g. merged "ice-cream")
//! - plain string    — let Stanza's model decide
//!
//! [`PatchedToken`] encodes this convention at the Rust↔Python boundary.
//! The [`Hint`](PatchedToken::Hint) variant is emitted only when the
//! character-DP merges multiple Stanza tokens into one and the merged text
//! looks like an English contraction ([`is_contraction`]). All other cases
//! produce [`Plain`](PatchedToken::Plain).
//!
//! # Provenance
//!
//! The character-position mapping algorithm replaces batchalign2's DP-based
//! `tokenizer_processor` (ud.py:610-700). Per-language MWT override rules
//! (BA2 `ud.py:662-695`) were audited and retired on 2026-04-21 after paired
//! probes showed every rule was dormant, redundant, or harmful; see
//! `book/src/reference/languages/{french,italian,portuguese,dutch}.md` for
//! the audit records. The character-DP alone satisfies the morphotag 1-to-1
//! invariant for the five previously-patched languages.
//!
//! See: `book/src/reference/morphotag-migration-audit.md` Section 6.

/// Token produced by [`align_tokens`] at the Rust↔Python boundary.
///
/// Encodes Stanza's tokenize-postprocessor MWT hint convention:
/// - `Plain(text)` — no hint, let Stanza's model decide
/// - `Hint(text, true)` — force MWT expansion
/// - `Hint(text, false)` — suppress MWT expansion
#[derive(Debug, Clone, PartialEq)]
pub enum PatchedToken {
    /// Plain string — no MWT hint.
    Plain(String),
    /// MWT hint tuple: `(text, should_expand)`.
    Hint(String, bool),
}

impl PatchedToken {
    /// Extract the text content regardless of variant.
    pub fn text(&self) -> &str {
        match self {
            PatchedToken::Plain(s) | PatchedToken::Hint(s, _) => s,
        }
    }
}

// ─── Contraction detection ──────────────────────────────────────────────────

/// Whether a merged token should be flagged as an English MWT contraction.
///
/// Replicates batchalign2's `ud.py` lines 680-685:
///   - Token contains `'`
///   - Language is English (`alpha2 == "en"`)
///   - The prefix before the first `'` is NOT `"o"` (excludes o'clock, o'er)
pub fn is_contraction(text: &str, alpha2: &str) -> bool {
    if !text.contains('\'') {
        return false;
    }
    if alpha2 != "en" {
        return false;
    }
    // Exclude o'clock, o'er, etc. — prefix before first apostrophe is "o"
    if let Some(prefix) = text.split('\'').next()
        && prefix.trim().to_lowercase() == "o"
    {
        return false;
    }
    true
}

// ─── Core alignment algorithm ───────────────────────────────────────────────

/// Align Stanza tokenizer output back to original CHAT words.
///
/// Uses character-position mapping: the concatenated characters of Stanza tokens
/// must equal the concatenated characters of original words (no reordering).
/// If they don't match, the function returns the Stanza tokens unchanged as
/// `PatchedToken::Plain` values.
///
/// After merging, language-specific MWT patches are applied (French, Italian,
/// Portuguese, Dutch).
///
/// # Arguments
///
/// - `original_words` — CHAT words (may contain shortening parens which are stripped)
/// - `stanza_tokens` — tokens from Stanza's neural tokenizer
/// - `alpha2` — ISO-639-1 language code (e.g. `"en"`, `"fr"`)
pub fn align_tokens(
    original_words: &[String],
    stanza_tokens: &[String],
    alpha2: &str,
) -> Vec<PatchedToken> {
    if stanza_tokens.is_empty() || original_words.is_empty() {
        return stanza_tokens
            .iter()
            .map(|t| PatchedToken::Plain(t.clone()))
            .collect();
    }

    // Clean original words (strip CHAT shortening parens)
    let cleaned: Vec<String> = original_words
        .iter()
        .map(|w| w.replace(['(', ')'], ""))
        .collect();

    // Build per-character maps: which word / which token does each char belong to?
    let mut ref_chars: Vec<usize> = Vec::new();
    for (word_idx, word) in cleaned.iter().enumerate() {
        for _ in word.chars() {
            ref_chars.push(word_idx);
        }
    }

    let mut tok_chars: Vec<usize> = Vec::new();
    for (tok_idx, tok) in stanza_tokens.iter().enumerate() {
        for _ in tok.chars() {
            tok_chars.push(tok_idx);
        }
    }

    // Character content must match — bail out if they don't
    let ref_str: String = cleaned.iter().flat_map(|w| w.chars()).collect();
    let tok_str: String = stanza_tokens.iter().flat_map(|t| t.chars()).collect();
    if ref_str != tok_str {
        return stanza_tokens
            .iter()
            .map(|t| PatchedToken::Plain(t.clone()))
            .collect();
    }

    // Build word->token mapping: for each original word, which token indices span it?
    let num_words = cleaned.len();
    let mut word_to_tokens: Vec<Vec<usize>> = vec![vec![]; num_words];
    for char_idx in 0..ref_chars.len() {
        let w = ref_chars[char_idx];
        let t = tok_chars[char_idx];
        if word_to_tokens[w].last() != Some(&t) {
            word_to_tokens[w].push(t);
        }
    }

    // 1-to-1 case: one Stanza token per CHAT word, no merge needed.
    if word_to_tokens.iter().all(|toks| toks.len() == 1) {
        return stanza_tokens
            .iter()
            .map(|t| PatchedToken::Plain(t.clone()))
            .collect();
    }

    // Merge case: at least one CHAT word spans multiple Stanza tokens.
    // Emit PatchedToken::Hint only when the merged text looks like an
    // English contraction (so Stanza's MWT processor expands it back).
    let num_tokens = stanza_tokens.len();
    let mut seen = vec![false; num_tokens];
    let mut tokens: Vec<PatchedToken> = Vec::with_capacity(num_words);

    for toks in &word_to_tokens {
        let unseen: Vec<usize> = toks.iter().copied().filter(|&i| !seen[i]).collect();
        if unseen.is_empty() {
            continue;
        }
        if unseen.len() == 1 {
            tokens.push(PatchedToken::Plain(stanza_tokens[unseen[0]].clone()));
        } else {
            let merged: String = unseen.iter().map(|&i| stanza_tokens[i].as_str()).collect();
            let is_contr = is_contraction(&merged, alpha2);
            tokens.push(PatchedToken::Hint(merged, is_contr));
        }
        for &i in &unseen {
            seen[i] = true;
        }
    }

    // Append any unmapped tokens (shouldn't happen with correct alignment).
    for (tok_idx, tok) in stanza_tokens.iter().enumerate() {
        if !seen[tok_idx] {
            tokens.push(PatchedToken::Plain(tok.clone()));
        }
    }

    tokens
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── is_contraction ────────────────────────────────────────────────

    #[test]
    fn test_english_contraction_detected() {
        assert!(is_contraction("don't", "en"));
        assert!(is_contraction("I'm", "en"));
        assert!(is_contraction("Claus'", "en"));
    }

    #[test]
    fn test_english_oclock_not_contraction() {
        assert!(!is_contraction("o'clock", "en"));
        assert!(!is_contraction("O'er", "en"));
    }

    #[test]
    fn test_non_english_not_contraction() {
        assert!(!is_contraction("l'homme", "fr"));
        assert!(!is_contraction("d'água", "pt"));
    }

    #[test]
    fn test_no_apostrophe_not_contraction() {
        assert!(!is_contraction("hello", "en"));
    }

    // ── align_tokens ─────────────────────────────────────────────────

    #[test]
    fn test_empty_inputs() {
        let result = align_tokens(&[], &[], "en");
        assert!(result.is_empty());
    }

    #[test]
    fn test_one_to_one_mapping() {
        let words = vec!["hello".into(), "world".into()];
        let tokens = vec!["hello".into(), "world".into()];
        let result = align_tokens(&words, &tokens, "en");
        assert_eq!(
            result,
            vec![
                PatchedToken::Plain("hello".into()),
                PatchedToken::Plain("world".into()),
            ]
        );
    }

    #[test]
    fn test_merge_compound() {
        // Stanza splits "ice-cream" into ["ice", "-", "cream"]
        let words = vec!["ice-cream".into()];
        let tokens = vec!["ice".into(), "-".into(), "cream".into()];
        let result = align_tokens(&words, &tokens, "en");
        assert_eq!(result, vec![PatchedToken::Hint("ice-cream".into(), false)]);
    }

    #[test]
    fn test_english_contraction_merge() {
        // Stanza splits "don't" into ["do", "n't"]
        let words = vec!["don't".into()];
        let tokens = vec!["do".into(), "n't".into()];
        let result = align_tokens(&words, &tokens, "en");
        assert_eq!(result, vec![PatchedToken::Hint("don't".into(), true)]);
    }

    #[test]
    fn test_character_mismatch_passthrough() {
        let words = vec!["hello".into()];
        let tokens = vec!["goodbye".into()];
        let result = align_tokens(&words, &tokens, "en");
        assert_eq!(result, vec![PatchedToken::Plain("goodbye".into())]);
    }

    #[test]
    fn test_shortening_parens_stripped() {
        // CHAT shortening: "(be)cause" → characters "because"
        let words = vec!["(be)cause".into()];
        let tokens = vec!["because".into()];
        let result = align_tokens(&words, &tokens, "en");
        assert_eq!(result, vec![PatchedToken::Plain("because".into())]);
    }

    // ── align_tokens: cross-language passthrough (no per-language rules) ──

    #[test]
    fn test_english_passthrough_no_patches() {
        let words = vec!["the".into(), "dog".into()];
        let tokens = vec!["the".into(), "dog".into()];
        let result = align_tokens(&words, &tokens, "en");
        assert_eq!(
            result,
            vec![
                PatchedToken::Plain("the".into()),
                PatchedToken::Plain("dog".into()),
            ]
        );
    }
}
