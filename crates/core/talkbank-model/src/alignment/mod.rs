//! Tier alignment module
//!
//! This module provides 1-1 alignment between tiers:
//! - Main tier → %mor tier items
//! - Main tier → %pho/%mod tier tokens (PhoAlignment)
//! - Main tier → %sin tier tokens
//! - %mor tier chunks → %gra tier relations
//!
//! # Alignment Rules
//!
//! ## Main → %mor Alignment
//! - Each alignable unit in main tier corresponds to exactly one %mor item
//! - Alignable content: words, tag markers, groups (recursively), replacements
//! - Excluded: retraces, repeats, scoped annotations themselves
//!
//! ## Main → %pho Alignment
//! - Each alignable unit in main tier corresponds to exactly one token
//! - Alignable content: words, tag markers, groups (recursively), replacements
//! - Count mismatches reported as E714 (too few) / E715 (too many)
//!
//! ## Main → %mod Alignment
//! - Same structural rules as %pho, but %mod = target/model pronunciation
//! - Uses the same `PhoAlignment` type but different error codes:
//!   E733 (too few) / E734 (too many)
//!
//! ## Main ↔ %wor (timing sidecar, not an alignment)
//! - `%wor` is **not** a structural alignment tier. It is a timing sidecar:
//!   bullets attached to the subset of main-tier words that passed the
//!   Wor-domain filter at `align` time.
//! - Expressed as [`WorTimingSidecar`] (see `wor.rs`), not a `TierAlignmentResult`.
//! - No positional indexing is defined when filtered counts differ — this
//!   is the normal state of any transcript edited without re-running `align`.
//! - Count mismatches are reported as [`WorTimingSidecar::Drifted`], never as
//!   `ParseError`s. `%wor` has no validation contract against the main tier.
//! - See KIB-016 in the VS Code extension backlog for the history of this
//!   reclassification.
//!
//! ## Main → %sin Alignment
//! - Each alignable unit in main tier corresponds to exactly one gesture/sign token
//! - Alignable content: same as %pho alignment
//!
//! ## %mor → %gra Alignment
//! - Each %mor chunk corresponds to exactly one %gra relation
//! - Chunks != Items: clitics create additional chunks!
//! - Example: `pro|it~v|be&PRES` creates 2 chunks (pre-clitic + main)
//!
//! # Example
//!
//! ```ignore
//! use talkbank_model::{MainTier, MorTier, Terminator, UtteranceContent, Word};
//! use talkbank_model::Span;
//! use talkbank_parser::parse_mor_tier;
//! use talkbank_model::alignment::align_main_to_mor;
//!
//! let main = MainTier::new(
//!     "CHI",
//!     vec![
//!         UtteranceContent::Word(Word::new_unchecked("hello", "hello")),
//!         UtteranceContent::Word(Word::new_unchecked("world", "world")),
//!     ],
//!     Terminator::Period { span: Span::DUMMY },
//! );
//!
//! let mor = match parse_mor_tier("v|hello n|world .") {
//!     Ok(mor) => mor,
//!     Err(_) => return,
//! };
//!
//! let alignment = align_main_to_mor(&main, &mor);
//! assert_eq!(alignment.pairs.len(), 3); // 2 words + 1 terminator
//! assert!(alignment.errors.is_empty());
//! ```
//!
//! # Implementation Notes
//!
//! - Alignment counts alignable content recursively
//! - %wor alignment mirrors %pho alignment rules
//! - New tiers must update `AlignmentSet` units and tests
//!
//! # Parse-Health Gating Rules
//!
//! - Alignment must consult utterance parse-health before reporting mismatch errors
//! - If a dependent tier domain is parse-tainted, suppress mismatch diagnostics for that domain pair
//! - Main-tier taint blocks main-dependent alignments but not unrelated dependent-dependent checks
//!   (e.g., %mor <-> %gra)
//! - Add targeted tests whenever alignment gating semantics change
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Word_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier>

mod format;
mod gra;
pub mod helpers;
/// Typed index-space newtypes for `%mor`/`%gra` alignment.
///
/// See the module-level docs: these types distinguish the four integer
/// "positions" that used to share `usize` and caused silent chunk↔item
/// confusion bugs (notably in the LSP `%gra` hover / highlight handlers
/// before 2026-04-16).
pub mod indices;
mod mor;
mod mor_gra_construction;
mod pho;
mod sin;
pub mod traits;
mod types;
mod wor;

#[cfg(test)]
mod location_tests;

// Re-export public API
pub use gra::{GraAlignment, GraAlignmentPair, align_mor_to_gra};
pub use helpers::{TierDomain, count_tier_positions_until};
pub use indices::{
    GraHeadRef, GraIndex, MainWordIndex, MorChunkIndex, MorItemIndex, PhoItemIndex,
    SemanticWordIndex1, SemanticWordIndexError, SinItemIndex,
};
pub use mor::{MorAlignment, align_main_to_mor};
pub use mor_gra_construction::{MorGraConstructionError, MorGraTerminatorSlot, try_align_mor_gra};
pub use pho::{PhoAlignment, align_main_to_pho};
pub use sin::{SinAlignment, align_main_to_sin};
pub use traits::{AlignableTier, IndexPair, TierAlignmentResult, TierCountable, positional_align};
pub use types::AlignmentPair;
pub use wor::{WorTimingSidecar, resolve_wor_timing_sidecar};
