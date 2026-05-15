//! Normalized word text for use as frequency-counting map keys.
//!
//! The transformation `word.cleaned_text().to_lowercase()` is the canonical
//! form used as map keys across all analysis commands. [`NormalizedWord`]
//! encapsulates this transformation so it is always applied consistently
//! across FREQ, MAXWD, DIST, COOCCUR, KWAL, COMBO, and all other commands
//! that need word-level deduplication or matching.
//!
//! [`clan_display_form()`] provides an alternative form that preserves `+` in
//! compound words (`ice+cream`) for CLAN-compatible output rendering.
//!
//! # Forward-compatibility
//!
//! As the grammar moves toward a looser `grammar.js` that pushes more checks
//! from parsing into validation, words that previously would not reach the AST
//! may start arriving as less-classified `Word` nodes. Centralizing the
//! normalization here means only [`NormalizedWord::from_word()`] needs updating
//! when that happens -- no command code changes are required.

use std::borrow::Borrow;
use std::fmt;

use serde::Serialize;
use talkbank_model::Word;

/// Lowercased, cleaned word text suitable for frequency counting.
///
/// Encapsulates `word.cleaned_text().to_lowercase()` — the canonical form
/// used as map keys across all analysis commands:
/// - `freq.rs` word counts
/// - `maxwd.rs` unique-word deduplication
/// - `dist.rs` per-word distribution tracking
/// - `cooccur.rs` word-pair keys
/// - `kwal.rs` / `combo.rs` keyword matching
///
/// # Using as a map key
///
/// ```
/// use std::collections::HashMap;
/// use clan_core::framework::NormalizedWord;
/// let mut map: HashMap<NormalizedWord, u64> = HashMap::new();
/// // `map.get("hello")` works because NormalizedWord: Borrow<str>
/// ```
///
/// # Invariant
///
/// The inner `String` is always lowercased and cleaned (CHAT markup stripped).
/// Never construct with `NormalizedWord(raw_string)` directly outside of this
/// module; always use `NormalizedWord::from_word`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct NormalizedWord(pub(crate) String);

impl NormalizedWord {
    /// Construct the canonical normalized form of a word.
    ///
    /// Applies `word.cleaned_text()` (strips CHAT markup / trailing punctuation)
    /// then `to_lowercase()`. This is the single authoritative normalization
    /// point for all analysis commands.
    ///
    /// # Precondition
    ///
    /// `word` must pass [`crate::framework::word_filter::is_countable_word`].
    /// Results are unspecified (but safe) for non-countable words.
    pub fn from_word(word: &Word) -> Self {
        NormalizedWord(word.cleaned_text().to_lowercase())
    }

    /// Return the normalized text as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Return the CLAN display form of a word (lowercased, overlap markers stripped).
///
/// CLAN preserves `+` in compound words (`ice+cream`, `choo+choo's`) and
/// uses lowercased raw text. Our `NormalizedWord` uses `cleaned_text()`
/// which strips `+`, so multiple commands need this alternative form for
/// CLAN-compatible output.
///
/// Note: CLAN `freq` is an exception — it preserves original case. Use
/// [`clan_display_form_preserve_case()`] for freq-style output.
pub fn clan_display_form(word: &Word) -> String {
    strip_overlap_markers(&word.raw_text().to_lowercase())
}

/// Return the CLAN display form of a word preserving original case.
///
/// Used by FREQ which displays words in their original casing.
pub fn clan_display_form_preserve_case(word: &Word) -> String {
    strip_overlap_markers(word.raw_text())
}

/// Strip CA overlap markers (⌈⌉⌊⌋ and indexed variants) from text.
fn strip_overlap_markers(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, '⌈' | '⌉' | '⌊' | '⌋'))
        .collect()
}

impl fmt::Display for NormalizedWord {
    /// Print the normalized token text without additional formatting.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for NormalizedWord {
    /// Expose the normalized token as `&str` for generic APIs.
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Enables `map.get("hello")` on `HashMap<NormalizedWord, _>` — no temporary
/// `NormalizedWord` allocation required at lookup sites.
impl Borrow<str> for NormalizedWord {
    /// Enable zero-allocation `&str` lookup against `NormalizedWord` map keys.
    fn borrow(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::Word;

    /// Construction from `Word` should lowercase and clean CHAT markup.
    #[test]
    fn normalized_word_lowercases() {
        let word = Word::simple("HELLO");
        let nw = NormalizedWord::from_word(&word);
        assert_eq!(nw.as_str(), "hello");
    }

    /// `Ord` should provide deterministic lexical ordering for map iteration.
    #[test]
    fn normalized_word_ord_for_map_ordering() {
        let a = NormalizedWord(String::from("apple"));
        let b = NormalizedWord(String::from("banana"));
        assert!(a < b);
    }

    /// `Borrow<str>` should allow `HashMap::get` with plain `&str` keys.
    #[test]
    fn borrow_enables_str_lookup() {
        use std::collections::HashMap;
        let mut map: HashMap<NormalizedWord, u64> = HashMap::new();
        map.insert(NormalizedWord(String::from("hello")), 42);
        // Lookup with &str — works via Borrow<str>
        assert_eq!(map.get("hello"), Some(&42));
    }
}
