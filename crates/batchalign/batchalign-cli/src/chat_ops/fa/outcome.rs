//! Forced alignment outcome vocabulary.
//!
//! Unlike morphotag, utseg, and coref — where a single pipeline call
//! produces one outcome per utterance — FA is structured as a multi-pass
//! pipeline with its own typed vocabulary per pass. This module gathers
//! the existing FA decision types in one place and documents how they
//! relate, completing Wave 5 of the morphotag reconciliation
//! architecture.
//!
//! ## The FA pass structure
//!
//! ```text
//! ┌──────────────────────────┐
//! │ UTR pre-pass             │  fa::utr::inject_utr_timing
//! │   (optional)             │  → UtrResult { injected, skipped, unmatched }
//! └──────────────────────────┘
//!               │
//!               ▼
//! ┌──────────────────────────┐
//! │ FA dispatch + response   │  alignment::parse_fa_response
//! │                          │  → Result<_, FaAlignmentError>
//! │                          │    (typed: JsonParse | IndexedCountMismatch)
//! └──────────────────────────┘
//!               │
//!               ▼
//! ┌──────────────────────────┐
//! │ Bullet repair post-pass  │  fa::repair::repair_bullets
//! │   (optional)             │  → RepairResult { stats, decisions }
//! └──────────────────────────┘
//! ```
//!
//! Each pass emits typed records that flow through the shared
//! [`DecisionRecord`](talkbank_transform::decisions::DecisionRecord) surface via
//! `DecisionModule::Fa`. Aggregate stats
//! ([`UtrResult`](super::utr::UtrResult),
//! [`RepairStats`](super::repair::RepairStats)) live on their respective
//! pass types for progress reporting; per-utterance provenance lives on
//! `DecisionRecord`.
//!
//! ## Why no single `FaOutcome` enum
//!
//! The morphotag/utseg/coref outcome enums have exactly one variant per
//! utterance because each pipeline makes a single dispatch decision per
//! utterance. FA makes multiple decisions per utterance — a UTR hint
//! may be injected, then a word-timing call may succeed or partially
//! fail, then a repair pass may average the bullet. Collapsing this
//! into one variant per utterance would lose the temporal structure.
//!
//! Instead, each pass continues to own its own vocabulary and emits
//! `DecisionRecord`s as side effects. The shared
//! [`DecisionModule::Fa`](talkbank_transform::decisions::DecisionModule::Fa) module
//! tag and the `strategy` string (e.g. `"end_clamped"`, `"gap_filled"`,
//! `"timing_stripped"`) give the downstream consumer a unified view.
//!
//! ## Typed-error boundary
//!
//! The only stringly-typed FA path before Wave 5 was
//! [`parse_fa_response`](super::alignment::parse_fa_response), which
//! returned `Result<_, String>`. It now returns
//! `Result<_, FaAlignmentError>` with two typed variants:
//!
//! - `JsonParse` — worker returned malformed JSON. Always a worker
//!   protocol bug.
//! - `IndexedCountMismatch` — worker returned the wrong number of
//!   per-word timings. The FA equivalent of morphotag's
//!   [`MisalignmentBug`](talkbank_transform::morphosyntax::MorOutcomeKind::MisalignmentBug).
//!
//! Call sites that were string-matching on the error substring
//! `"length mismatch"` now match on the variant directly — see
//! `fa::tests::test_parse_fa_response_indexed_length_mismatch_rejected`.
//!
//! ## Re-exports
//!
//! This module re-exports the FA decision types for call sites that
//! want a single `use crate::chat_ops::fa::outcome::*` bring-in.

pub use super::alignment::FaAlignmentError;
pub use super::repair::{RepairDecision, RepairResult, RepairStats};
pub use super::utr::UtrResult;
