//! Batchalign-specific CHAT orchestration: forced alignment, speaker
//! reassignment, morphosyntax/NLP runtime glue, and the cache-key newtypes
//! that gate Python worker results.
//!
//! Pure deterministic CHAT/text logic (parse, serialize, extract, inject,
//! diff, dp_align, asr_postprocess, build_chat, validate, compare,
//! benchmark, merge_abbreviations, constituency, tokenizer_realign,
//! utseg_compute, and the canonical `coref` / `translate` / `utseg` task
//! surfaces) lives in `talkbank-transform`. Import those directly from
//! `talkbank_transform`; this crate no longer re-exports them.
//!
//! P8 fold (2026-04-30): this module is the merged form of the former
//! `batchalign-chat-ops` crate, now living inside the `batchalign` crate
//! as `batchalign::chat_ops`. The `batchalign-types` boundary stays
//! separate to keep the pyo3 maturin build slim.
//!
//! # Module map
//!
//! | Module          | Responsibility                                                                            |
//! |-----------------|-------------------------------------------------------------------------------------------|
//! | [`fa`]          | Forced alignment: utterance grouping, DP alignment, timing injection, UTR, monotonicity   |
//! | [`speaker`]     | Speaker code mapping and diarization-driven reassignment                                  |
//! | [`morphosyntax`]| Stanza-coupled `%mor`/`%gra` orchestration (payload collection, dispatch, injection glue) |
//! | [`nlp`]         | UD types, UD→CHAT mapping helpers, language-specific rules                                |
//! | [`cache_key`]   | BLAKE3-keyed [`CacheKey`] / [`CacheTaskName`] newtypes for FA + UTR cache                 |

pub mod cache_key;
pub mod fa;
pub mod morphosyntax_ops;
pub mod nlp;
pub mod speaker;

// Re-export newtypes used by all NLP task modules and the server orchestrators.
pub use cache_key::{CacheKey, CacheTaskName};

// Re-export talkbank_model types commonly needed by downstream crates
// (e.g. batchalign-server) that shouldn't depend on talkbank_model directly.
pub use talkbank_model::ParseError;
pub use talkbank_model::Span;
pub use talkbank_model::alignment::helpers::TierDomain;
pub use talkbank_model::header::Header;
pub use talkbank_model::model::BulletContent;
pub use talkbank_model::model::{
    ChatFile, DependentTier, LanguageCode, Line, Linker, UserDefinedDependentTier, Utterance,
};
pub use talkbank_model::{UtteranceIdx, WordIdx};
