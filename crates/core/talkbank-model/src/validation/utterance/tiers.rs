//! Dependent-tier validation rules.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#CA_Overlaps>
//! - <https://talkbank.org/0info/manuals/CHAT.html#CA_Delimiters>
//! - <https://talkbank.org/0info/manuals/CHAT.html#QuotationFollows_Linker>
//! - <https://talkbank.org/0info/manuals/CHAT.html#QuotedNewLine_Terminator>

use crate::model::Utterance;
use crate::{ErrorCode, ErrorSink, ParseError, Severity};
use std::collections::HashSet;

/// Enforce "one tier per type" for dependent tiers within an utterance.
pub(crate) fn check_no_duplicate_dependent_tiers(utterance: &Utterance, errors: &impl ErrorSink) {
    let mut seen_tiers: HashSet<&str> = HashSet::new();

    for tier in &utterance.dependent_tiers {
        let kind = tier.kind();

        if !seen_tiers.insert(kind) {
            // Duplicate tier found — report at the tier's own span
            errors.report(
                ParseError::at_span(
                    ErrorCode::DuplicateDependentTier,
                    Severity::Error,
                    tier.span(),
                    format!(
                        "Duplicate dependent tier: %{} appears more than once for this utterance",
                        kind
                    ),
                )
                .with_suggestion(format!("Remove or merge the duplicate %{} tier", kind)),
            );
        }
    }
}
