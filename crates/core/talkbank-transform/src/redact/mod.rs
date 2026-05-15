//! Structural CHAT-file sanitizer.
//!
//! Strips contributor lexical content from a parsed `ChatFile` while
//! preserving timing, structure, speaker codes, and CHAT validity. The
//! output is intended for engineering use (debugging, validator
//! reproduction, structural analysis) where the original transcript is
//! protected by contributor consent and cannot be sent through commercial
//! LLM tooling.
//!
//! # Quick Start
//!
//! ```no_run
//! use talkbank_transform::redact::{sanitize, SanitizationPolicy};
//! # fn parse_input() -> talkbank_model::ChatFile { unimplemented!() }
//! let input = parse_input();
//! let policy = SanitizationPolicy::strict();
//! let sanitized = sanitize(input, &policy).expect("structurally sound");
//! let chat_text = sanitized.to_chat_string();
//! ```
//!
//! # What Is Preserved vs. Replaced
//!
//! See the crate the `talkbank-tools` book chapter `user-guide/sanitize.md` for the full leak-surface inventory. The
//! short version:
//!
//! - **Preserved byte-exact**: timing bullets, `%wor` per-word offsets,
//!   speaker codes, `@Languages`, `@Birth`, `@Date`, `@Media`, `@PID`,
//!   `@L1Of`, structural markers (`+`, `~`, CA elements, `@n`, POS tags).
//! - **Replaced**: every `WordContent::Text`, `Shortening` text, `%mor`
//!   lemmas, `%pho`/`%sin`/`%mod` tiers (dropped), free-text dependent
//!   tiers, free-text headers (`@Comment`, `@Transcriber`, ...),
//!   `@Participants` names, `@ID` `custom_field`/`education`, free-text
//!   annotations.
//!
//! # Determinism + Idempotence
//!
//! Placeholder generation is keyed off `(utterance_index, word_index)`
//! tree position rather than a global counter. Sanitizing the same input
//! always produces byte-identical output, and re-sanitizing a sanitized
//! file is a no-op.
//!
//! Out of scope for v1: speaker-code anonymization, `@Birth`/`@Date`
//! fuzzing, `@Media` filename redaction, audio-side sanitization,
//! unsanitize/round-trip mapping. See the `talkbank-tools` book chapter `user-guide/sanitize.md` for the full inventory.

mod dependent_tier;
mod document;
mod error;
mod header;
mod placeholder;
mod policy;
mod word;

/// Marker text emitted in place of redacted free-text content.
pub(crate) const REDACTED_TEXT: &str = "[redacted]";

pub use document::{SanitizedDocument, sanitize};
pub use error::RedactError;
pub use placeholder::{PlaceholderIndex, PlaceholderToken};
pub use policy::SanitizationPolicy;
