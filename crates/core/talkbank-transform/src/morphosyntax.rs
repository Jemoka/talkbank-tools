//! Per-utterance morphotag outcome classification.
//!
//! The morphotag pipeline emits exactly one [`MorOutcome`] per utterance it
//! visits. The outcome is a typed statement of what happened, not a
//! side-effect of logging or an implicit silent skip:
//!
//! - [`MorOutcomeKind::NotApplicable`] — the utterance had zero Mor-alignable
//!   words under CHAT policy (fillers, fragments, untranscribed material
//!   only), so no `%mor` tier is produced. This is correct, expected
//!   behavior, not a failure.
//! - [`MorOutcomeKind::Aligned`] — Stanza returned exactly N tokens for N
//!   CHAT words after MWT reassembly; `%mor` / `%gra` were injected.
//! - [`MorOutcomeKind::MisalignmentBug`] — the `|stanza_tokens| = |chat_words|`
//!   invariant was violated. This is a bug in one of extraction, Stanza
//!   realignment, MWT reassembly, or the terminator filter. The diagnostic
//!   carries enough data to triage the stage without re-running.
//!
//! The invariant is deterministic by construction:
//!
//! 1. CHAT-side extraction uses
//!    [`counts_for_tier`](talkbank_model::alignment::helpers::counts_for_tier)
//!    to yield exactly N alignable Mor-domain words.
//! 2. The Python worker sets
//!    `tok_ctx.original_words = word_lists`
//!    (`python/batchalign/inference/morphosyntax.py:348-349`), so Stanza's
//!    tokenizer realigns to the pre-specified CHAT word boundaries.
//! 3. MWT expansions are signaled via Range token IDs and reassembled
//!    1-chunk-per-CHAT-word in `nlp::mapping::map_ud_sentence`.
//!
//! When all three cooperate, the count matches by construction.
//! [`MisalignmentBug`](MorOutcomeKind::MisalignmentBug) is therefore
//! never silently absorbed — it is typed, logged, and surfaced through
//! [`DecisionRecord`](crate::decisions::DecisionRecord) so operators
//! see it and developers can fix it.
//!
//! See `book/src/architecture/morphotag-invariants.md` for the full
//! architectural discussion.
//!
//! # Module layout
//!
//! The morphosyntax surface is large enough that it lives in submodules:
//!
//! - [`types`] — small mapping types: `MwtDict`, `MappingContext`, `lang2`,
//!   `MappingError`, `TokenizationMode`, `MultilingualPolicy`.
//! - [`ud_types`] — UD value types: `UniversalPos`, `DepRel`, `VerbForm`,
//!   feature constants, `UdId`, `UdPunctable`, `UdWord`, `UdSentence`,
//!   `UdResponse`, plus `validate_and_clean` / `is_bogus_lemma` /
//!   `sanitize_mor_text`.
//! - [`payload`] — payload collection (`collect_payloads`, `declared_languages`,
//!   `MorphosyntaxBatchItem`, `BatchItemWithPosition`, `AlignmentWarning`,
//!   `PayloadCollection`) and `%mor`/`%gra` mutation passes
//!   (`clear_morphosyntax`, `validate_mor_alignment`, `prepare_text`, etc.).
//! - [`pos_hints`] — `apply_pos_hints` + `HintOutcome`, `is_stanza_supported`
//!   + `supported_iso3_codes`.
//! - [`outcome`] — `MorOutcome`, `MorOutcomeKind`, `NotApplicableReason`,
//!   `classify_not_applicable`.
//!
//! Every public item in those submodules is re-exported below so existing
//! consumers (`batchalign`, `batchalign-chat-ops`, the CLI, PyO3) keep
//! their `talkbank_transform::morphosyntax::<name>` import paths working.

pub use crate::inject::{MisalignmentClass, MisalignmentDiagnostic};

mod features;
mod gra_validate;
mod injection;
mod invariants;
pub mod l2;
mod lang_en;
mod lang_fr;
mod lang_it;
mod lang_ja;
mod mapping_helpers;
mod mapping_provenance;
mod mor_word;
mod outcome;
mod payload;
mod pos_hints;
mod sentence_mapping;
mod stanza_raw;
mod synthesis;
#[cfg(test)]
mod tests;
mod types;
mod ud_types;

pub use gra_validate::validate_generated_gra;
pub use injection::{InjectionResult, RetokenizationInfo, inject_results};
pub use invariants::*;
pub use lang_en::is_irregular;
pub use lang_fr::{french_pronoun_case, is_apm_noun};
pub use lang_it::{try_handle_italian_range_override, try_handle_italian_single_override};
pub use lang_ja::{JaOverride, japanese_verbform};
pub use mapping_helpers::{assemble_mors, normalize_deprel, provenance_for_ud_word};
pub use mapping_provenance::{ChunkHead, ChunkProvenance, MorProvenance};
pub use mor_word::{clean_lemma, is_clitic, map_ud_word_to_mor};
pub use outcome::{MorOutcome, MorOutcomeKind, NotApplicableReason, classify_not_applicable};
pub use payload::{
    AlignmentWarning, BatchItemWithPosition, MorphosyntaxBatchItem, PayloadCollection,
    clear_morphosyntax, clear_morphosyntax_selective, collect_payloads, declared_languages,
    prepare_text, remove_empty_morphosyntax_placeholders, validate_mor_alignment,
};
pub use pos_hints::{HintOutcome, apply_pos_hints, is_stanza_supported, supported_iso3_codes};
pub use sentence_mapping::{
    TerminatorPolicy, build_gra_and_validate, is_terminator_punct, map_ud_sentence,
    map_ud_sentence_expanded, map_ud_sentence_with_overrides,
};
pub use stanza_raw::*;
pub use synthesis::synthesize_special_form_mor;
pub use types::{
    MappingContext, MappingError, MultilingualPolicy, MwtDict, TokenizationMode, lang2,
};
pub use ud_types::{
    DepRel, FINITE_COPULA_PRES_3SG, PRESENT_PARTICIPLE, UdId, UdPunctable, UdResponse, UdSentence,
    UdWord, UniversalPos, VerbForm, has_key_value, has_verb_form_fin, is_bogus_lemma,
    sanitize_mor_text, validate_and_clean,
};
