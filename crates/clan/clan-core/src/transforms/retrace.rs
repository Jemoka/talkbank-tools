//! RETRACE -- add `%ret:` dependent tier with verbatim main-tier copy.
//!
//! Reimplements CLAN's `retrace` command, which adds a `%ret:` dependent
//! tier to each utterance containing a verbatim serialized copy of the
//! main-tier content (including retrace markers, pauses, events, etc.).
//! This serves as a reference tier preserving the original utterance text
//! before other transforms modify it.
//!
//! See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409318)
//! for the original command documentation.
//!
//! All headers are preserved. Existing dependent tiers are kept. The `%ret:`
//! tier is inserted at position 0 (before other dependent tiers).
//!
//! # Differences from CLAN
//!
//! - Operates on the typed AST rather than raw text line scanning.
//! - Uses the framework transform pipeline (parse → transform → serialize).
//! - Generates `%ret:` content by serializing the main tier via `WriteChat`
//!   (AST → text), ensuring the preserved text is structurally faithful
//!   rather than copying raw input lines that may have formatting quirks.

use talkbank_model::{ChatFile, DependentTier, Line, NonEmptyString, UserDefinedDependentTier};

use crate::framework::{TransformCommand, TransformError};

/// RETRACE transform: add `%ret:` tier copying main tier content verbatim.
pub struct RetraceCommand;

impl TransformCommand for RetraceCommand {
    type Config = ();

    /// Insert `%ret:` tiers containing verbatim serialized main-tier content.
    fn transform(&self, file: &mut ChatFile) -> Result<(), TransformError> {
        for line in file.lines.iter_mut() {
            if let Line::Utterance(utterance) = line {
                let content = utterance.main.content.to_content_string();

                if let Some(content) = NonEmptyString::new(&content) {
                    // "ret" is a compile-time-non-empty literal —
                    // `NonEmptyString::new` cannot return `None`.
                    #[allow(clippy::expect_used)]
                    let ret_tier = DependentTier::UserDefined(UserDefinedDependentTier {
                        label: NonEmptyString::new("ret").expect("label is non-empty"),
                        content,
                        span: talkbank_model::Span::DUMMY,
                    });

                    // Insert %ret right after the main tier (before other dependent tiers)
                    utterance.dependent_tiers.insert(0, ret_tier);
                }
            }
        }

        Ok(())
    }
}
