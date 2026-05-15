//! Small public types that surface at the morphosyntax mapping API
//! boundary: language-code helpers, the typed mapping error, and the
//! two policy enums that callers (`batchalign`, CLI, PyO3) drive
//! the pipeline with.
//!
//! Sized to read in one sitting. The richer UD value types (UPOS,
//! DepRel, VerbForm, UdWord) live in [`super::ud_types`]; the
//! payload-collection types in [`super::payload`].

use std::collections::BTreeMap;

use talkbank_model::model::LanguageCode;

/// Alias for the MWT lexicon: surface form → expansion tokens.
pub type MwtDict = BTreeMap<String, Vec<String>>;

/// Context for UD-to-CHAT morphosyntax mapping and language-specific rewrites.
pub struct MappingContext {
    /// Language code used to select language-specific override rules.
    pub lang: LanguageCode,
}

/// Normalize a language code to its 2-letter form.
///
/// The pipeline passes 3-letter ISO 639-3 codes ("eng", "fra", "jpn"), but
/// some language-specific logic is keyed by 2-letter ISO 639-1 codes ("en",
/// "fr", "ja"). Unknown codes are returned unchanged.
pub fn lang2(code: &str) -> &str {
    match code {
        "eng" => "en",
        "fra" | "fre" => "fr",
        "jpn" => "ja",
        "deu" | "ger" => "de",
        "ita" => "it",
        "spa" => "es",
        "por" => "pt",
        "zho" | "cmn" | "chi" => "zh",
        "heb" => "he",
        "ara" => "ar",
        "nld" | "dut" => "nl",
        "cat" => "ca",
        s if s.len() <= 2 => s,
        s => s,
    }
}

/// Structured error type for UD-to-CHAT mapping failures.
#[derive(Debug, thiserror::Error)]
pub enum MappingError {
    /// A word produced an empty MOR stem after lemma cleaning and sanitization.
    #[error("Empty MOR stem: word={word:?}, lemma={lemma:?}, upos={upos:?}")]
    EmptyStem {
        /// Original word form.
        word: String,
        /// Lemma after cleaning.
        lemma: String,
        /// Universal POS tag.
        upos: String,
    },

    /// The generated %gra tier has a circular dependency.
    #[error("Circular dependency in generated %gra: {details}")]
    CircularDependency {
        /// Description of the cycle.
        details: String,
    },

    /// The generated %gra tier has an invalid head reference.
    #[error("Invalid head reference in generated %gra: {details}")]
    InvalidHeadReference {
        /// Description of the invalid reference.
        details: String,
    },

    /// Generated %mor and %gra have mismatched chunk counts.
    #[error("%mor has {mor_chunks} chunks but %gra has {gra_count} relations")]
    ChunkCountMismatch {
        /// Number of %mor chunks.
        mor_chunks: usize,
        /// Number of %gra relations.
        gra_count: usize,
    },

    /// The generated %gra tier has no root or multiple roots.
    #[error("Invalid root structure in generated %gra: {details}")]
    InvalidRoot {
        /// Description of the root problem.
        details: String,
    },

    /// A UD word has a deprel value that cannot produce a valid CHAT %gra relation.
    #[error("Invalid deprel in UD parse: {details}")]
    InvalidDeprel {
        /// Description of the invalid deprel.
        details: String,
    },

    /// `assemble_mors` was called with an empty component slice.
    #[error("assemble_mors called with empty components — structural bug in caller")]
    EmptyRangeComponents,
}

/// Controls whether the morphosyntax pipeline retokenizes using Stanza's
/// neural tokenizer or preserves original CHAT word boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TokenizationMode {
    /// Preserve original CHAT tokenization.
    Preserve,
    /// Allow Stanza retokenization to rewrite CHAT words.
    StanzaRetokenize,
}

impl From<bool> for TokenizationMode {
    fn from(retokenize: bool) -> Self {
        if retokenize {
            Self::StanzaRetokenize
        } else {
            Self::Preserve
        }
    }
}

/// Controls whether utterances marked with a non-primary language are processed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MultilingualPolicy {
    /// Process all utterances regardless of `@s` language marking.
    ProcessAll,
    /// Skip utterances whose `@s` language marker differs from the primary file language.
    SkipNonPrimary,
}

impl MultilingualPolicy {
    /// Convert from the legacy boolean flag used at CLI and PyO3 boundaries.
    pub fn from_skip_flag(skip: bool) -> Self {
        if skip {
            Self::SkipNonPrimary
        } else {
            Self::ProcessAll
        }
    }

    /// Whether non-primary-language utterances should be skipped.
    pub fn should_skip_non_primary(self) -> bool {
        matches!(self, Self::SkipNonPrimary)
    }
}
