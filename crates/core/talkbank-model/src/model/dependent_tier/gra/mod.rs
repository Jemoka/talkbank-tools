//! Grammatical relations tier (%gra) representation.
//!
//! The %gra tier provides dependency syntax annotations using Universal Dependencies
//! relations. It specifies grammatical relationships between morphological chunks
//! in the %mor tier.
//!
//! This module combines relation labels, edge triples, and tier-level structural
//! checks so `%gra` parsing and serialization share one contract surface.
//!
//! # Format
//!
//! Each relation has the format:
//! ```text
//! word_index|head_index|relation_type
//! ```
//!
//! Where:
//! - **word_index**: Position in %mor tier (1-indexed)
//! - **head_index**: Position of syntactic parent (0 = ROOT of sentence)
//! - **relation_type**: Universal Dependencies relation (e.g., SUBJ, OBJ, ROOT)
//!
//! # Dependency Structure
//!
//! The %gra tier creates a dependency tree where:
//! - Each word points to its syntactic head (parent)
//! - The main verb typically has head_index 0 (ROOT)
//! - All other words connect through dependency relations
//!
//! # Tier Type
//!
//! - **%gra**: Grammatical relations for %mor tier
//!
//! # CHAT Manual Reference
//!
//! - [Grammatical Relations](https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier)
//! - [Universal Dependencies](https://universaldependencies.org/)
//!
//! Note: The legacy CLAN MOR manual has been removed from the TalkBank
//! website. GRA now follows UD-oriented conventions documented in the
//! CHAT manual.
//!
//! # Example
//!
//! ```text
//! *CHI: I eat cookies .
//! %mor: pro:sub|I v|eat n|cookie-PL .
//! %gra: 1|2|SUBJ 2|0|ROOT 3|2|OBJ 4|2|PUNCT
//! ```
//!
//! This represents the dependency tree:
//! ```text
//!        eat (2, ROOT)
//!       /   |   \
//!      /    |    \
//!   I(1) cookies(3) .(4)
//!  SUBJ    OBJ     PUNCT
//! ```

mod relation;
mod relation_type;
mod tier;
mod tier_type;

// Re-export all public types
pub use relation::GrammaticalRelation;
pub use relation_type::GrammaticalRelationType;
pub use tier::{GraTier, validate_gra_structure};
pub use tier_type::GraTierType;
