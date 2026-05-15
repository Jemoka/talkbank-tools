//! Granular tier parsing functions organized by linguistic category
//!
//! This module provides specialized parsing functions for each tier type,
//! organized by the linguistic category they represent.
//!
//! **Note:** These functions take tree-sitter `Node` arguments and are primarily
//! for parser use. For general tier parsing, use `parse_dependent_tier()`
//! which accepts tier content strings directly.
//!
//! # Examples
//!
//! Parse tiers using the high-level API:
//! ```
//! use talkbank_parser::parse_dependent_tier;
//! use talkbank_model::ErrorCollector;
//! use talkbank_model::ParseOutcome;
//!
//! let errors = ErrorCollector::new();
//! let mor = parse_dependent_tier("%mor:\tn|hello det|the .", &errors);
//! let gra = parse_dependent_tier("%gra:\t1|2|SUBJ 2|0|ROOT", &errors);
//! assert!(matches!(mor, ParseOutcome::Parsed(_)));
//! assert!(matches!(gra, ParseOutcome::Parsed(_)));
//! ```
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Phonology_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Comment_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Explanation_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Addressee_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Situation_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Intonation_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Coding_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Speech_Act>

pub mod action;
pub mod grammar;
pub mod morphology;
pub mod phonology;
pub mod text;

// Re-export all tier parsing functions at the tiers level
pub use action::{parse_act_tier, parse_cod_tier, parse_sin_tier};
pub use grammar::parse_gra_tier;
pub use morphology::parse_mor_tier;
pub use phonology::{parse_mod_tier_from_unparsed, parse_pho_tier};
pub use text::{
    parse_add_tier, parse_com_tier, parse_exp_tier, parse_gpx_tier, parse_int_tier, parse_sit_tier,
    parse_spa_tier,
};
