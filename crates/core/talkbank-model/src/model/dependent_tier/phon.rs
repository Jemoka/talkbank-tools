//! Phon project extension tiers: `%modsyl`, `%phosyl`, `%phoaln`.
//!
//! These tiers originate from the [Phon](https://www.phon.ca/) phonological
//! analysis tool and provide syllable-annotated phonological transcription and
//! segmental alignment between target (model) and actual (phone) IPA forms.
//!
//! # Tier Types
//!
//! | CHAT Tier  | Phon Internal Name   | Aligns With            |
//! |------------|---------------------|------------------------|
//! | `%modsyl`  | `TargetSyllables`   | `%mod` (content-based) |
//! | `%phosyl`  | `ActualSyllables`   | `%pho` (content-based) |
//! | `%phoaln`  | `PhoneAlignment`    | `%mod` & `%pho` (positional, word-by-word) |
//!
//! # Format Examples
//!
//! Syllabified target (each segment has `phoneme:PositionCode`):
//! ```text
//! %modsyl:    ˈb:Oe:Ns:Ct:R m:Oɔ̃:N
//! ```
//!
//! Syllabified actual:
//! ```text
//! %phosyl:    ˈb:Oe:Nt͡j:Oe:Nĭ:Ns:C
//! ```
//!
//! Phone alignment (source↔target pairs, comma within word, space between words):
//! ```text
//! %phoaln:    a↔a,p↔p b↔b,ɛ↔ɛ,t↔t̪
//! ```
//!
//! # Alignment Semantics
//!
//! - **%modsyl → %mod**: Stripping position codes (`:N`, `:O`, `:C`, etc.) and
//!   stress markers (`ˈ`, `ˌ`) from %modsyl should yield the same phonemes as %mod.
//! - **%phosyl → %pho**: Same content-based alignment as %modsyl → %mod.
//! - **%phoaln → %mod & %pho**: Word N in %phoaln aligns with word N in both
//!   %mod and %pho. `∅` represents insertions/deletions.
//!
//! Reference: Phon CHAT Extension Tier Alignment specification.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

use crate::Span;
use crate::model::NonEmptyString;

// ---------------------------------------------------------------------------
// Syllabified phonology tier (%modsyl, %phosyl)
// ---------------------------------------------------------------------------

/// Which flavour of syllabified phonology tier this is.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
pub enum SylTierType {
    /// `%modsyl` — syllabified target/model pronunciation.
    Modsyl,
    /// `%phosyl` — syllabified actual/phone production.
    Phosyl,
}

/// A syllabified phonology tier (`%modsyl` or `%phosyl`).
///
/// Content is organized as space-separated **words**, each containing
/// IPA phonemes annotated with syllable position codes
/// (`phoneme:Position` pairs, e.g. `b:Oɛ:Nt:C`).
///
/// Position codes observed in the Phon data:
/// - `N` — Nucleus (vowel center)
/// - `O` — Onset (syllable-initial consonant)
/// - `C` — Coda (syllable-final consonant)
/// - `D` — Left appendix
/// - `E` — Ambisyllabic / right appendix
/// - `R` — Rime / rhotic
///
/// Stress markers (`ˈ` primary, `ˌ` secondary) may precede any segment.
///
/// # Alignment
///
/// Each word aligns 1-to-1 with a word in the corresponding phonological
/// tier (`%mod` for modsyl, `%pho` for phosyl). Stripping position codes
/// and stress markers yields the raw phonemes which must match the
/// corresponding tier's content.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct SylTier {
    /// Which tier this is (Modsyl or Phosyl).
    pub tier_type: SylTierType,

    /// Syllabified words (space-separated in CHAT serialization).
    ///
    /// Each word is a raw string containing `phoneme:Position` sequences.
    /// Full segment-level parsing of these strings is deferred — the word
    /// boundary structure is sufficient for alignment validation.
    pub words: Vec<NonEmptyString>,

    /// Source span for error reporting.
    #[serde(skip)]
    #[schemars(skip)]
    pub span: Span,
}

impl SylTier {
    /// Creates a new syllabified tier from pre-split words.
    pub fn new(tier_type: SylTierType, words: Vec<NonEmptyString>) -> Self {
        Self {
            tier_type,
            words,
            span: Span::DUMMY,
        }
    }

    /// Sets source span metadata.
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = span;
        self
    }

    /// Returns the number of syllabified words.
    pub fn word_count(&self) -> usize {
        self.words.len()
    }

    /// Returns the CHAT tier prefix.
    ///
    /// Currently outputs `%xmodsyl` / `%xphosyl` to match the Phon project's
    /// existing convention. When the tiers are officially adopted into CHAT
    /// (dropping the `x` prefix), update this to `%modsyl` / `%phosyl`.
    pub fn prefix(&self) -> &'static str {
        match self.tier_type {
            SylTierType::Modsyl => "%xmodsyl",
            SylTierType::Phosyl => "%xphosyl",
        }
    }
}

impl std::fmt::Display for SylTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut first = true;
        for word in &self.words {
            if !first {
                write!(f, " ")?;
            }
            write!(f, "{}", word)?;
            first = false;
        }
        Ok(())
    }
}

impl super::WriteChat for SylTier {
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        write!(w, "{}:\t{}", self.prefix(), self)
    }
}

// ---------------------------------------------------------------------------
// Phone alignment tier (%phoaln)
// ---------------------------------------------------------------------------

/// A single segment alignment pair from `%phoaln`.
///
/// Represents the mapping of one phonological segment (from %mod/modsyl)
/// to one phonetic segment (from %pho/phosyl). `None` represents the null
/// symbol `∅`, indicating an insertion or deletion.
///
/// # Format
///
/// `source↔target` where either side may be `∅`:
/// - `a↔a` — identity mapping
/// - `ɪ↔ɛ` — substitution (lowering)
/// - `∅↔ʔ` — insertion (epenthesis)
/// - `b↔∅` — deletion (elision)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct AlignmentPair {
    /// Source segment (from target/model), `None` = `∅` (insertion).
    pub source: Option<NonEmptyString>,
    /// Target segment (from actual/phone), `None` = `∅` (deletion).
    pub target: Option<NonEmptyString>,
}

impl std::fmt::Display for AlignmentPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (&self.source, &self.target) {
            (Some(s), Some(t)) => write!(f, "{}↔{}", s, t),
            (Some(s), None) => write!(f, "{}↔∅", s),
            (None, Some(t)) => write!(f, "∅↔{}", t),
            (None, None) => write!(f, "∅↔∅"),
        }
    }
}

/// Word-level alignment: a sequence of segment alignment pairs.
///
/// Corresponds to one word position in the utterance. Pairs are
/// comma-separated in CHAT serialization.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct WordAlignment {
    /// Segment-level alignment pairs for this word.
    pub pairs: Vec<AlignmentPair>,
}

impl std::fmt::Display for WordAlignment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut first = true;
        for pair in &self.pairs {
            if !first {
                write!(f, ",")?;
            }
            write!(f, "{}", pair)?;
            first = false;
        }
        Ok(())
    }
}

/// Phone alignment tier (`%phoaln`).
///
/// Provides a segmental alignment between the target (model) and actual
/// (phone) IPA transcriptions, organized word-by-word.
///
/// # Format
///
/// `source↔target` pairs are comma-separated within a word, and words
/// are space-separated:
/// ```text
/// %phoaln:    a↔a,p↔p b↔b,ɛ↔ɛ,t↔t̪
/// ```
///
/// The null symbol `∅` marks insertions (source=∅) or deletions (target=∅).
///
/// # Alignment
///
/// Word N in %phoaln aligns positionally with word N in both %mod and %pho.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct PhoalnTier {
    /// Per-word alignment data.
    pub words: Vec<WordAlignment>,

    /// Source span for error reporting.
    #[serde(skip)]
    #[schemars(skip)]
    pub span: Span,
}

impl PhoalnTier {
    /// Creates a new phone alignment tier from pre-parsed word alignments.
    pub fn new(words: Vec<WordAlignment>) -> Self {
        Self {
            words,
            span: Span::DUMMY,
        }
    }

    /// Sets source span metadata.
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = span;
        self
    }

    /// Returns the number of aligned words.
    pub fn word_count(&self) -> usize {
        self.words.len()
    }
}

impl std::fmt::Display for PhoalnTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut first = true;
        for word in &self.words {
            if !first {
                write!(f, " ")?;
            }
            write!(f, "{}", word)?;
            first = false;
        }
        Ok(())
    }
}

impl super::WriteChat for PhoalnTier {
    /// Serializes as `%xphoaln:` to match Phon's current convention.
    /// When officially adopted into CHAT, update to `%phoaln:`.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        write!(w, "%xphoaln:\t{}", self)
    }
}

// ---------------------------------------------------------------------------
// Parsing helpers
// ---------------------------------------------------------------------------

/// Parse a `%phoaln` content string into word alignments.
///
/// Format: space-separated words, each word has comma-separated `source↔target`
/// pairs where either side may be `∅`.
pub fn parse_phoaln_content(content: &str) -> Result<Vec<WordAlignment>, PhoalnParseError> {
    let mut words = Vec::new();

    for word_str in content.split_whitespace() {
        let mut pairs = Vec::new();
        for pair_str in word_str.split(',') {
            let pair = parse_alignment_pair(pair_str)?;
            pairs.push(pair);
        }
        if pairs.is_empty() {
            return Err(PhoalnParseError::EmptyWord);
        }
        words.push(WordAlignment { pairs });
    }

    Ok(words)
}

/// Parse a single `source↔target` alignment pair.
fn parse_alignment_pair(s: &str) -> Result<AlignmentPair, PhoalnParseError> {
    // The ↔ character is U+2194 (LEFT RIGHT ARROW), 3 bytes in UTF-8
    let Some(arrow_pos) = s.find('↔') else {
        return Err(PhoalnParseError::MissingArrow(s.to_string()));
    };

    let source_str = &s[..arrow_pos];
    let target_str = &s[arrow_pos + '↔'.len_utf8()..];

    let source = if source_str == "∅" || source_str.is_empty() {
        None
    } else {
        Some(NonEmptyString::new(source_str).ok_or(PhoalnParseError::EmptySegment)?)
    };

    let target = if target_str == "∅" || target_str.is_empty() {
        None
    } else {
        Some(NonEmptyString::new(target_str).ok_or(PhoalnParseError::EmptySegment)?)
    };

    Ok(AlignmentPair { source, target })
}

/// Errors from parsing `%phoaln` content.
#[derive(Debug, Clone, thiserror::Error)]
pub enum PhoalnParseError {
    /// Missing `↔` separator in an alignment pair.
    #[error("missing '↔' separator in alignment pair: {0}")]
    MissingArrow(String),
    /// Empty word (no alignment pairs).
    #[error("empty word in alignment (no pairs)")]
    EmptyWord,
    /// Empty segment string (not ∅, just empty).
    #[error("empty segment string in alignment pair")]
    EmptySegment,
}

/// Parse `%modsyl` or `%phosyl` content into word strings.
///
/// Simply splits on whitespace to get word-level boundaries.
/// Within-word segment parsing (position codes) is deferred.
pub fn parse_syl_content(content: &str) -> Vec<NonEmptyString> {
    content
        .split_whitespace()
        .filter_map(NonEmptyString::new)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_phoaln() {
        let words = parse_phoaln_content("a↔a,p↔p b↔b,ɛ↔ɛ,t↔t̪").unwrap();
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].pairs.len(), 2);
        assert_eq!(words[1].pairs.len(), 3);
        assert_eq!(words[0].to_string(), "a↔a,p↔p");
        assert_eq!(words[1].to_string(), "b↔b,ɛ↔ɛ,t↔t̪");
    }

    #[test]
    fn parse_phoaln_with_null_segments() {
        let words = parse_phoaln_content("∅↔ʔ,æ̃↔ʌ̃,n↔n ð↔d,æ↔æ,t↔tʰ").unwrap();
        assert_eq!(words.len(), 2);
        assert!(words[0].pairs[0].source.is_none());
        assert_eq!(words[0].pairs[0].target.as_ref().unwrap().as_str(), "ʔ");
    }

    #[test]
    fn parse_phoaln_deletion() {
        let words = parse_phoaln_content("b↔∅").unwrap();
        assert_eq!(words[0].pairs[0].source.as_ref().unwrap().as_str(), "b");
        assert!(words[0].pairs[0].target.is_none());
    }

    #[test]
    fn roundtrip_phoaln() {
        let input = "a↔a,p↔p b↔b,ɛ↔ɛ,t↔t̪";
        let words = parse_phoaln_content(input).unwrap();
        let tier = PhoalnTier::new(words);
        assert_eq!(tier.to_string(), input);
    }

    #[test]
    fn roundtrip_phoaln_with_nulls() {
        let input = "∅↔ʔ,æ̃↔ʌ̃ b↔∅";
        let words = parse_phoaln_content(input).unwrap();
        let tier = PhoalnTier::new(words);
        assert_eq!(tier.to_string(), input);
    }

    #[test]
    fn parse_syl_words() {
        let words = parse_syl_content("ˈb:Oe:Ns:Ct:R m:Oɔ̃:N");
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].as_str(), "ˈb:Oe:Ns:Ct:R");
        assert_eq!(words[1].as_str(), "m:Oɔ̃:N");
    }

    #[test]
    fn syl_tier_roundtrip() {
        let words = parse_syl_content("ˈb:Oe:Ns:Ct:R m:Oɔ̃:N");
        let tier = SylTier::new(SylTierType::Modsyl, words);
        assert_eq!(tier.to_string(), "ˈb:Oe:Ns:Ct:R m:Oɔ̃:N");

        let mut chat = String::new();
        super::super::WriteChat::write_chat(&tier, &mut chat).unwrap();
        assert_eq!(chat, "%xmodsyl:\tˈb:Oe:Ns:Ct:R m:Oɔ̃:N");
    }

    #[test]
    fn phoaln_write_chat() {
        let words = parse_phoaln_content("a↔a,p↔p").unwrap();
        let tier = PhoalnTier::new(words);
        let mut chat = String::new();
        super::super::WriteChat::write_chat(&tier, &mut chat).unwrap();
        assert_eq!(chat, "%xphoaln:\ta↔a,p↔p");
    }

    #[test]
    fn missing_arrow_error() {
        let result = parse_phoaln_content("a,b");
        assert!(result.is_err());
    }
}
