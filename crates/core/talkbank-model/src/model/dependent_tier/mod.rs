//! Dependent tier types (% lines).
//!
//! Dependent tiers provide additional annotation layers that align with or
//! describe main tier utterances. Each tier type serves a specific analytical
//! purpose in language research.
//!
//! # Major Tier Categories
//!
//! **Linguistic annotation tiers (structured):**
//! - **%mor**: Morphological analysis (part-of-speech, stems, affixes)
//! - **%gra**: Grammatical relations (Universal Dependencies syntax)
//! - **%pho**: Phonological transcription (IPA, UNIBET)
//! - **%sin**: Gesture and sign language annotation
//!
//! **Descriptive tiers (with inline bullets):**
//! - **%act**: Action descriptions with timing
//! - **%cod**: Behavioral coding categories
//! - **%com**: Comments and notes
//! - **%exp**: Explanations of unclear utterances
//! - **%gpx**: Gesture descriptions with timing
//! - **%sit**: Situational context
//! - **%spa**: Speech act labels
//!
//! **Phon project tiers (phonological syllabification and alignment):**
//! - **%modsyl**: Syllabified target pronunciation (aligns with %mod)
//! - **%phosyl**: Syllabified actual pronunciation (aligns with %pho)
//! - **%phoaln**: Segmental alignment between target and actual IPA
//!
//! **Text-only tiers (simple string content):**
//! - **%alt**: Alternative transcriptions
//! - **%eng**: English translations
//! - **%err**: Error annotations
//! - **%fac**: Facial expressions
//! - **%ort**: Orthographic representations
//!
//! # Alignment Types
//!
//! **Word-by-word alignment** (%mor, %gra, %pho, %sin):
//! - One token per alignable main tier word
//! - Excludes pauses, events, retraces
//! - Terminator gets own token
//!
//! **Inline bullets** (%act, %cod, %com, etc.):
//! - Free-form text with embedded timing markers
//! - Bullets use format: `\u0015START_END\u0015`
//! - Picture references: `\u0015%pic:"filename"\u0015`
//!
//! **Phonological alignment** (%modsyl, %phosyl, %phoaln):
//! - %modsyl/%phosyl: Content-based alignment with %mod/%pho (strip syllable
//!   position codes → must equal corresponding tier content)
//! - %phoaln: Word-by-word positional alignment with both %mod and %pho
//! - IPA phoneme content is treated as opaque (same strategy as %pho/%mod)
//!
//! **No alignment** (text-only tiers):
//! - Simple string content
//! - No structural alignment required
//!
//! # CHAT Format Examples
//!
//! Morphological tier:
//! ```text
//! *CHI: I want cookies .
//! %mor: pro:sub|I v|want n|cookie-PL .
//! ```
//!
//! Grammatical relations tier:
//! ```text
//! %gra: 1|2|SUBJ 2|0|ROOT 3|2|OBJ 4|2|PUNCT
//! ```
//!
//! Action tier with bullets:
//! ```text
//! %act: picks up toy 1000_2000 then drops it 3000_4000
//! ```
//!
//! Comment tier:
//! ```text
//! %com: Child is pointing to picture %pic:"toy.jpg"
//! ```
//!
//! # References
//!
//! - [CHAT Manual: Dependent Tiers](https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers)
//! - [CHAT Manual: Morphological Tier](https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier)
//! - [CHAT Manual: Grammatical Relations](https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier)

mod act;
mod bullet_content;
mod cod;
mod gra;
mod kind;
pub mod mor;
pub mod pho;
mod phon;
pub mod sin;
mod text;
mod tim;
mod types;
pub mod wor;

pub use act::*;
pub use bullet_content::*;
pub use cod::*;
pub use gra::*;
pub use mor::*;
pub use pho::*;
pub use phon::*;
pub use sin::*;
pub use text::*;
pub use tim::TimTier;
pub use types::{DependentTier, TextTier, UserDefinedDependentTier};
pub use wor::*;

// Re-export for submodules via super::
pub(crate) use crate::model::WriteChat;
