//! Shared framework replacing CLAN's CUTT infrastructure.
//!
//! CUTT (CLAN Utility Template Toolkit) is the 17,926-line C framework that handles
//! file I/O, argument parsing, speaker filtering, and command dispatch for all CLAN
//! commands. Since talkbank-tools already handles file I/O, parsing, and AST construction,
//! our framework only needs to handle the command-specific parts.
//!
//! See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html) for the original
//! CLAN command semantics that this framework reimplements.
//!
//! ## Analysis Framework
//!
//! For read-only commands that compute statistics over CHAT files:
//!
//! - [`AnalysisCommand`] — Trait that each analysis command implements
//! - [`FilterConfig`] — Speaker/tier/word/gem filtering (replaces CUTT's `+t`/`-t`, `+s`/`-s`, `+g`/`-g`)
//! - [`UtteranceRange`] — Typed CLAN-style utterance ranges (`+z25-125`)
//! - [`DiscoveredChatFiles`] — Shared CHAT-file discovery for file/directory analysis targets
//! - [`AnalysisRunner`] — File loading, filtering, and command dispatch
//! - [`AnalysisResult`] and [`OutputFormat`] — Output formatting (text, JSON, CSV, CLAN-compat)
//!
//! ## Transform Framework
//!
//! For commands that modify CHAT files in place:
//!
//! - [`TransformCommand`] — Trait for file-modifying commands (FLO, LOWCASE, etc.)
//! - [`run_transform()`] — Pipeline: parse --> transform --> serialize --> write
//!
//! ## Supporting Modules
//!
//! - [`NormalizedWord`] — Canonical lowercased word form for frequency-counting keys
//! - [`word_filter`] — Predicates for identifying countable words (skipping fillers, fragments, etc.)

mod chat_ast;
mod cod;
mod command;
pub mod domain_types;
mod filter;
mod id_filter;
mod input;
pub mod mor;
mod normalized_word;
mod output;
mod runner;
pub mod transform;
pub mod word_filter;

pub use chat_ast::{
    count_main_scoped_errors, dependent_tier_content_text, gra_relation_texts, mor_item_has_verb,
    mor_item_morpheme_count, mor_item_pos_tags, mor_item_texts, spoken_content_text,
    spoken_main_text,
};
pub use cod::{
    CodSemanticElement, CodSemanticItem, CodSemanticTier, cod_item_values, cod_semantic_tier,
};
pub use command::{AnalysisCommand, FileContext};
pub use domain_types::{
    CodeDepth, FrequencyThreshold, GemLabel, KeywordPattern, OverlapThreshold, TierKind,
    UtteranceLimit, WordLimit, WordPattern,
};
pub use filter::{
    FilterConfig, GemFilter, ParseUtteranceRangeError, SpeakerFilter, TierFilter, UtteranceRange,
    WordFilter, parse_utterance_range,
};
pub use id_filter::{IdFilter, IdPattern, IdPatternField, ParseIdFilterError, parse_id_filter};
pub use input::DiscoveredChatFiles;
pub use normalized_word::{NormalizedWord, clan_display_form, clan_display_form_preserve_case};
pub use output::{AnalysisResult, CommandOutput, OutputFormat, Section, TableRow};
pub use runner::{AnalysisRunner, RunnerError};
pub use transform::{TransformCommand, TransformError, run_transform};
pub use word_filter::{
    countable_words, countable_words_in_utterance, countable_words_in_utterance_with_retracings,
    countable_words_with_retracings, has_countable_words, is_countable_word, word_pattern_matches,
};

// ── Shared domain type aliases ──────────────────────────────────────
//
// These clarify what bare `u64`/`f64` represent in struct fields and
// function signatures across multiple commands.

/// Count of utterances processed or scored.
pub type UtteranceCount = u64;

/// Count of word tokens.
pub type WordCount = u64;

/// Count of unique word types (distinct forms).
pub type TypeCount = u64;

/// Count of morphemes (from %mor tier).
pub type MorphemeCount = u64;

/// Count of speaker turns.
pub type TurnCount = u64;

/// Grammatical category count (POS tag occurrences like nouns, verbs, etc.).
pub type POSCount = u64;

/// Count of analysis scoring points.
pub type ScorePoints = u32;

/// Count of distinct speakers observed.
pub type SpeakerCount = usize;

/// Analysis score (computed floating-point metric like MLU, TTR, DSS, D-value).
pub type AnalysisScore = f64;
