//! COMBTIER -- combine multiple dependent tiers of the same type into one.
//!
//! Reimplements CLAN's COMBTIER command, which merges all instances of a
//! specified dependent tier within each utterance into a single combined tier.
//! When multiple tiers of the same type exist on an utterance (e.g., two
//! `%com:` tiers), their content is concatenated with a configurable separator.
//!
//! This is useful for cleaning up files where duplicate tiers were introduced
//! during manual editing or automated annotation.
//!
//! # Differences from CLAN
//!
//! - Operates on the typed AST rather than raw text line scanning.
//! - Uses the framework transform pipeline (parse → transform → serialize).
//! - Matches tiers by typed `DependentTier` variant labels instead of
//!   string-matching raw `%`-prefixed lines.
//! - Combined result is stored as a user-defined tier to preserve the
//!   concatenated text faithfully.

use talkbank_model::{BulletContent, ChatFile, DependentTier, Line, NonEmptyString};

use crate::framework::{TransformCommand, TransformError, dependent_tier_content_text};

/// Configuration for the COMBTIER command.
pub struct CombtierConfig {
    /// The tier label to combine (e.g., "com" for %com, "spa" for %spa).
    pub tier: String,
    /// Separator between combined tier contents (default: " ").
    pub separator: String,
}

/// COMBTIER transform: combine dependent tiers of the same type.
pub struct CombtierCommand {
    config: CombtierConfig,
}

impl CombtierCommand {
    /// Create a new COMBTIER command.
    pub fn new(config: CombtierConfig) -> Self {
        Self { config }
    }
}

impl TransformCommand for CombtierCommand {
    type Config = CombtierConfig;

    /// Merge duplicate dependent tiers of the configured type within each utterance.
    fn transform(&self, file: &mut ChatFile) -> Result<(), TransformError> {
        let tier_label = self.config.tier.to_lowercase();

        for line in file.lines.iter_mut() {
            if let Line::Utterance(utt) = line {
                let mut combined_texts: Vec<String> = Vec::new();
                let mut indices_to_remove: Vec<usize> = Vec::new();

                // Collect matching tier content and track indices
                for (idx, dep) in utt.dependent_tiers.iter().enumerate() {
                    if tier_matches(dep, &tier_label) {
                        combined_texts.push(tier_text_content(dep));
                        indices_to_remove.push(idx);
                    }
                }

                // If we found 2+ matching tiers, combine them into one
                if combined_texts.len() > 1 {
                    let combined = combined_texts.join(&self.config.separator);

                    // Remove all but the first matching tier (in reverse order)
                    for &idx in indices_to_remove[1..].iter().rev() {
                        utt.dependent_tiers.remove(idx);
                    }

                    // Replace the first matching tier's content with the combined text
                    if let Some(&first_idx) = indices_to_remove.first()
                        && let Some(tier) = utt.dependent_tiers.get_mut(first_idx)
                    {
                        replace_tier_content(tier, &combined);
                    }
                }
            }
        }
        Ok(())
    }
}

/// Check if a dependent tier matches the target tier label.
fn tier_matches(tier: &DependentTier, label: &str) -> bool {
    match tier {
        DependentTier::Com(_) => label == "com",
        DependentTier::Act(_) => label == "act",
        DependentTier::Cod(_) => label == "cod",
        DependentTier::Exp(_) => label == "exp",
        DependentTier::Gpx(_) => label == "gpx",
        DependentTier::Sit(_) => label == "sit",
        DependentTier::Spa(_) => label == "spa",
        DependentTier::Alt(_) => label == "alt",
        DependentTier::Eng(_) => label == "eng",
        DependentTier::Err(_) => label == "err",
        DependentTier::Flo(_) => label == "flo",
        DependentTier::Ort(_) => label == "ort",
        DependentTier::Par(_) => label == "par",
        DependentTier::UserDefined(u) => u.label.as_str().to_lowercase() == label,
        _ => false,
    }
}

/// Extract text content from a dependent tier.
fn tier_text_content(tier: &DependentTier) -> String {
    match tier {
        DependentTier::Alt(t)
        | DependentTier::Eng(t)
        | DependentTier::Err(t)
        | DependentTier::Flo(t)
        | DependentTier::Ort(t)
        | DependentTier::Par(t) => t.as_str().to_owned(),
        DependentTier::UserDefined(u) => u.content.as_str().to_owned(),
        other => dependent_tier_content_text(other),
    }
}

/// Replace the content of a dependent tier while preserving its tier type when possible.
fn replace_tier_content(tier: &mut DependentTier, new_content: &str) {
    let new_bullet_content = BulletContent::from_text(new_content);

    match tier {
        DependentTier::Com(t) => t.content = new_bullet_content,
        DependentTier::Act(t) => t.content = new_bullet_content,
        DependentTier::Cod(t) => t.content = new_bullet_content,
        DependentTier::Exp(t) => t.content = new_bullet_content,
        DependentTier::Gpx(t) => t.content = new_bullet_content,
        DependentTier::Sit(t) => t.content = new_bullet_content,
        DependentTier::Spa(t) => t.content = new_bullet_content,
        DependentTier::Add(t) => t.content = new_bullet_content,
        DependentTier::Int(t) => t.content = new_bullet_content,
        DependentTier::Alt(t)
        | DependentTier::Eng(t)
        | DependentTier::Err(t)
        | DependentTier::Flo(t)
        | DependentTier::Ort(t)
        | DependentTier::Par(t) => {
            if let Some(ne) = NonEmptyString::new(new_content) {
                t.content = ne;
            }
        }
        DependentTier::UserDefined(u) => {
            if let Some(ne) = NonEmptyString::new(new_content) {
                u.content = ne;
            }
        }
        // Caller-contract invariant: `replace_tier_content` is only
        // invoked for the tier kinds enumerated above; the caller
        // (within this transform) filters tiers by kind before
        // calling.
        #[allow(clippy::unreachable)]
        _ => unreachable!("replace_tier_content called with unsupported tier kind"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::Span;
    use talkbank_model::{
        CodTier, ComTier, MainTier, Terminator, Utterance, UtteranceContent, Word,
    };

    fn make_utterance() -> Utterance {
        let content = vec![UtteranceContent::Word(Box::new(Word::simple("hello")))];
        let main = MainTier::new("CHI", content, Terminator::Period { span: Span::DUMMY });
        Utterance::new(main)
    }

    #[test]
    fn combtier_preserves_cod_tier_type() {
        let mut utt = make_utterance();
        utt.dependent_tiers
            .push(DependentTier::Cod(CodTier::from_text("$A")));
        utt.dependent_tiers
            .push(DependentTier::Cod(CodTier::from_text("$B")));
        let mut file = ChatFile::new(vec![Line::Utterance(Box::new(utt))]);

        CombtierCommand::new(CombtierConfig {
            tier: "cod".to_owned(),
            separator: " ".to_owned(),
        })
        .transform(&mut file)
        .expect("combtier should succeed");

        let Line::Utterance(utt) = &file.lines[0] else {
            panic!("expected utterance");
        };
        assert_eq!(utt.dependent_tiers.len(), 1);
        match &utt.dependent_tiers[0] {
            DependentTier::Cod(tier) => assert_eq!(tier.to_chat(), "%cod:\t$A $B"),
            other => panic!("expected cod tier, got {other:?}"),
        }
    }

    #[test]
    fn combtier_preserves_com_tier_type() {
        let mut utt = make_utterance();
        utt.dependent_tiers
            .push(DependentTier::Com(ComTier::from_text("one")));
        utt.dependent_tiers
            .push(DependentTier::Com(ComTier::from_text("two")));
        let mut file = ChatFile::new(vec![Line::Utterance(Box::new(utt))]);

        CombtierCommand::new(CombtierConfig {
            tier: "com".to_owned(),
            separator: " | ".to_owned(),
        })
        .transform(&mut file)
        .expect("combtier should succeed");

        let Line::Utterance(utt) = &file.lines[0] else {
            panic!("expected utterance");
        };
        assert_eq!(utt.dependent_tiers.len(), 1);
        match &utt.dependent_tiers[0] {
            DependentTier::Com(tier) => assert_eq!(tier.to_chat(), "%com:\tone | two"),
            other => panic!("expected com tier, got {other:?}"),
        }
    }
}
