//! Conversation Analysis (CA) prosodic markers used inside words.
//!
//! This module re-exports two marker families:
//! - single-point markers ([`CAElement`])
//! - paired scope delimiters ([`CADelimiter`])
//!
//! Together they model the CHAT CA symbol inventory while keeping parsing,
//! validation, and serialization logic centralized in one namespace.
//!
//! # CA Elements
//!
//! Individual prosodic markers that appear at a single point:
//! - Pitch: `↑` (up), `↓` (down), `↻` (reset)
//! - Other: `≠` (blocked), `∾` (constriction), `⁑` (hardening), etc.
//!
//! # CA Delimiters
//!
//! Paired prosodic markers that scope over a region of text:
//! - Speech rate: ∆ (faster), ∇ (slower)
//! - Volume/register: ° (softer), ▁ (low pitch), ▔ (high pitch), ◉ (louder)
//! - Voice quality: ∬ (whisper), ♋ (breathy), ☺ (smile voice), etc.
//!
//! # CHAT Format Examples
//!
//! **Elements (individual markers):**
//! ```text
//! *CHI: ↑hello .                         # Pitch rise on "hello"
//! *CHI: I ↓know .                        # Pitch fall on "know"
//! *INV: ≠wait .                          # Blocked segment
//! ```
//!
//! **Delimiters (paired markers):**
//! ```text
//! *CHI: I want ∆that∆ .                  # Faster speech on "that"
//! *MOT: °okay° .                         # Softer speech on "okay"
//! *CHI: ∬thank you∬ .                    # Whisper voice on "thank you"
//! ```
//!
//! **Combined usage:**
//! ```text
//! *CHI: ↑°really°↓ ?                     # Rise + soft + fall
//! *MOT: ∆very fast∆ .                    # Faster speech span
//! ```
//!
//! # Delimiter Pairing
//!
//! CA delimiters should be balanced - each opening delimiter must have a corresponding
//! closing delimiter. This is enforced during validation (error code E230).
//!
//! # References
//!
//! - [CA Subwords (elements)](https://talkbank.org/0info/manuals/CHAT.html#CA_Subwords)
//! - [CA Delimiters](https://talkbank.org/0info/manuals/CHAT.html#CA_Delimiters)

mod delimiters;
mod elements;

#[cfg(test)]
mod tests;

pub use delimiters::{CADelimiter, CADelimiterType};
pub use elements::{CAElement, CAElementType};
