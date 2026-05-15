//! TRIM -- remove selected dependent tiers from a CHAT file.
//!
//! The local CLAN manual describes `trim` as a shorthand for removing coding
//! tiers while preserving the rest of the transcript structure. For example:
//!
//! | CLAN command | Rust equivalent |
//! |---|---|
//! | `trim -t%mor file.cha +1` | `chatter clan trim file.cha --exclude-tier mor` |
//! | `trim -t%* file.cha +1` | `chatter clan trim file.cha --exclude-tier '*'` |
//!
//! `clan-core` implements this as AST-native dependent-tier filtering.

use talkbank_model::{ChatFile, DependentTier, Line};

use crate::framework::{TransformCommand, TransformError};

/// Configuration for the TRIM transform.
#[derive(Debug, Clone, Default)]
pub struct TrimConfig {
    /// Dependent tiers to keep. If empty, all tiers are initially kept.
    pub include_tiers: Vec<String>,
    /// Dependent tiers to remove after include filtering.
    pub exclude_tiers: Vec<String>,
}

/// TRIM transform: remove selected dependent tiers.
pub struct TrimCommand {
    /// Configuration.
    pub config: TrimConfig,
}

impl TransformCommand for TrimCommand {
    type Config = TrimConfig;

    fn transform(&self, file: &mut ChatFile) -> Result<(), TransformError> {
        let include = normalize_patterns(&self.config.include_tiers)?;
        let exclude = normalize_patterns(&self.config.exclude_tiers)?;

        if include.is_empty() && exclude.is_empty() {
            return Err(TransformError::Transform(
                "TRIM requires --tier and/or --exclude-tier to specify dependent tiers".into(),
            ));
        }

        for line in &mut file.lines {
            if let Line::Utterance(utt) = line {
                utt.dependent_tiers
                    .retain(|tier| should_keep_tier(tier, &include, &exclude));
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TierPattern {
    Any,
    Exact(String),
}

fn normalize_patterns(raw_patterns: &[String]) -> Result<Vec<TierPattern>, TransformError> {
    raw_patterns.iter().map(|p| normalize_pattern(p)).collect()
}

fn normalize_pattern(raw: &str) -> Result<TierPattern, TransformError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(TransformError::Transform(
            "empty TRIM tier selector is not allowed".into(),
        ));
    }

    let trimmed = trimmed.strip_prefix('%').unwrap_or(trimmed);
    if trimmed == "*" {
        return Ok(TierPattern::Any);
    }

    let lowered = trimmed.to_ascii_lowercase();
    let normalized = match lowered.as_str() {
        "trn" => "mor",
        "grt" => "gra",
        other => other,
    };

    Ok(TierPattern::Exact(normalized.to_owned()))
}

fn should_keep_tier(
    tier: &DependentTier,
    include: &[TierPattern],
    exclude: &[TierPattern],
) -> bool {
    let kind = tier.kind().to_ascii_lowercase();

    let included = if include.is_empty() {
        true
    } else {
        include
            .iter()
            .any(|pattern| matches_pattern(pattern, &kind))
    };

    included
        && !exclude
            .iter()
            .any(|pattern| matches_pattern(pattern, &kind))
}

fn matches_pattern(pattern: &TierPattern, kind: &str) -> bool {
    match pattern {
        TierPattern::Any => true,
        TierPattern::Exact(exact) => exact == kind,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::{
        BulletContent, ChatFile, CodTier, ComTier, DependentTier, Header, MainTier, MorTier,
        Terminator, Utterance, Word,
    };

    fn main_tier() -> MainTier {
        MainTier::new(
            "CHI",
            vec![talkbank_model::UtteranceContent::Word(Box::new(
                Word::simple("hello"),
            ))],
            Terminator::Period {
                span: talkbank_model::Span::DUMMY,
            },
        )
    }

    fn sample_file() -> ChatFile {
        let utt = Utterance::new(main_tier())
            .add_dependent_tier(DependentTier::Mor(MorTier::new_mor(
                vec![],
                talkbank_model::Terminator::Period {
                    span: talkbank_model::Span::DUMMY,
                },
            )))
            .add_dependent_tier(DependentTier::Cod(CodTier::from_text("$A")))
            .add_dependent_tier(DependentTier::Com(ComTier {
                content: BulletContent::from_text("note"),
                span: talkbank_model::Span::DUMMY,
            }));

        ChatFile::new(vec![
            Line::header(Header::Begin),
            Line::utterance(utt),
            Line::header(Header::End),
        ])
    }

    #[test]
    fn trim_excludes_specific_tier() {
        let mut file = sample_file();
        let cmd = TrimCommand {
            config: TrimConfig {
                include_tiers: vec![],
                exclude_tiers: vec!["mor".into()],
            },
        };

        cmd.transform(&mut file).expect("trim should succeed");

        let utt = file.lines[1].as_utterance().expect("utterance");
        assert_eq!(utt.dependent_tiers.len(), 2);
        assert!(
            !utt.dependent_tiers
                .iter()
                .any(|t| matches!(t, DependentTier::Mor(_)))
        );
        assert!(
            utt.dependent_tiers
                .iter()
                .any(|t| matches!(t, DependentTier::Cod(_)))
        );
    }

    #[test]
    fn trim_excludes_all_dependent_tiers_with_wildcard() {
        let mut file = sample_file();
        let cmd = TrimCommand {
            config: TrimConfig {
                include_tiers: vec![],
                exclude_tiers: vec!["*".into()],
            },
        };

        cmd.transform(&mut file).expect("trim should succeed");

        let utt = file.lines[1].as_utterance().expect("utterance");
        assert!(utt.dependent_tiers.is_empty());
    }

    #[test]
    fn trim_includes_only_selected_tier() {
        let mut file = sample_file();
        let cmd = TrimCommand {
            config: TrimConfig {
                include_tiers: vec!["cod".into()],
                exclude_tiers: vec![],
            },
        };

        cmd.transform(&mut file).expect("trim should succeed");

        let utt = file.lines[1].as_utterance().expect("utterance");
        assert_eq!(utt.dependent_tiers.len(), 1);
        assert!(matches!(utt.dependent_tiers[0], DependentTier::Cod(_)));
    }

    #[test]
    fn trim_supports_trn_and_grt_aliases() {
        let mut file = sample_file();
        let cmd = TrimCommand {
            config: TrimConfig {
                include_tiers: vec![],
                exclude_tiers: vec!["trn".into()],
            },
        };

        cmd.transform(&mut file).expect("trim should succeed");

        let utt = file.lines[1].as_utterance().expect("utterance");
        assert!(
            !utt.dependent_tiers
                .iter()
                .any(|t| matches!(t, DependentTier::Mor(_)))
        );
    }

    #[test]
    fn trim_errors_without_tier_selection() {
        let mut file = sample_file();
        let cmd = TrimCommand {
            config: TrimConfig::default(),
        };

        let err = cmd.transform(&mut file).expect_err("trim should fail");
        assert!(
            err.to_string()
                .contains("requires --tier and/or --exclude-tier")
        );
    }
}
