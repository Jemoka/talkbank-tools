//! Synthesis layer for `%mor` entries on words NLP cannot meaningfully
//! analyze (special-form `@<letter>` markers).
//!
//! Stanza has no abstain output; on a non-word it confabulates a real
//! analysis. Rather than over-write Stanza's guess slot-by-slot, this
//! module synthesizes the entire `%mor` entry from `FormType` + the
//! surface form. The pre-Stanza `xbxxx` placeholder substitution in
//! `payload.rs` ensures Stanza never sees the non-word and so produces
//! a clean parse for surrounding words.

mod synthesize;
mod table;

pub use synthesize::synthesize_special_form_mor;
