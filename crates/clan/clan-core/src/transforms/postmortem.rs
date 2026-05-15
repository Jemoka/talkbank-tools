//! POSTMORTEM -- pattern-matching rules for `%mor` post-processing.
//!
//! Reimplements CLAN's POSTMORTEM command, which applies pattern-matching
//! and replacement rules to dependent tiers (typically `%mor:`). Rules are
//! applied sequentially, and wildcard tokens (`*`) match any single token.
//! The replacement side uses `$-` to copy the matched wildcard text.
//!
//! See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409261)
//! for the original command documentation.
//!
//! # External data
//!
//! Requires a rules file (default: `postmortem.cut`).
//! Format: `from_pattern => to_replacement` (one rule per line, using `=>`
//! or `==>` as the separator). Lines starting with `#` or `;` are comments.
//!
//! # Differences from CLAN
//!
//! - When a `%mor` tier is modified, the result is stored as a user-defined
//!   tier to preserve the modified text (since structured `%mor` tiers have
//!   their own internal representation).

use std::path::{Path, PathBuf};

use talkbank_model::{ChatFile, DependentTier, Line};

use crate::framework::{
    TransformCommand, TransformError, dependent_tier_content_text, mor_item_texts,
};

/// Configuration for the POSTMORTEM command.
pub struct PostmortemConfig {
    /// Path to the rules file.
    pub rules_path: PathBuf,
    /// Target tier label (default: "mor").
    pub target_tier: String,
}

/// A single replacement rule.
#[derive(Debug, Clone)]
struct Rule {
    /// Pattern to match (space-separated tokens).
    from: Vec<String>,
    /// Replacement tokens.
    to: Vec<String>,
}

/// POSTMORTEM transform: apply pattern-matching rules to a tier.
pub struct PostmortemCommand {
    rules: Vec<Rule>,
    target_tier: String,
}

impl PostmortemCommand {
    /// Create a new POSTMORTEM command, loading rules from file.
    pub fn new(config: PostmortemConfig) -> Result<Self, TransformError> {
        let rules = load_rules(&config.rules_path)?;
        Ok(Self {
            rules,
            target_tier: config.target_tier,
        })
    }
}

impl TransformCommand for PostmortemCommand {
    type Config = PostmortemConfig;

    /// Apply pattern-matching rules to the target tier on each utterance.
    fn transform(&self, file: &mut ChatFile) -> Result<(), TransformError> {
        for line in file.lines.iter_mut() {
            if let Line::Utterance(utt) = line {
                // Find the target tier and apply rules
                for dep in utt.dependent_tiers.iter_mut() {
                    let is_target = match dep {
                        DependentTier::Mor(_) if self.target_tier == "mor" => true,
                        DependentTier::UserDefined(u)
                            if u.label.as_str().eq_ignore_ascii_case(&self.target_tier) =>
                        {
                            true
                        }
                        _ => false,
                    };

                    if is_target {
                        match dep {
                            DependentTier::Mor(tier) => {
                                let content = mor_item_texts(tier).join(" ");
                                let modified = apply_rules(&content, &self.rules);
                                if modified != content {
                                    return Err(TransformError::Transform(
                                        "POSTMORTEM would rewrite a typed %mor tier, but \
                                         clan-core does not support degrading %mor into \
                                         raw text. Apply POSTMORTEM only to text/user-defined \
                                         tiers until an AST-based %mor rewrite exists."
                                            .to_owned(),
                                    ));
                                }
                            }
                            _ => {
                                let content = dependent_tier_content_text(dep);
                                let modified = apply_rules(&content, &self.rules);
                                if modified != content
                                    && let (Some(label), Some(new_content)) = (
                                        talkbank_model::NonEmptyString::new(&self.target_tier),
                                        talkbank_model::NonEmptyString::new(&modified),
                                    )
                                {
                                    *dep = DependentTier::UserDefined(
                                        talkbank_model::UserDefinedDependentTier {
                                            label,
                                            content: new_content,
                                            span: talkbank_model::Span::DUMMY,
                                        },
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

/// Apply replacement rules to tier content text.
fn apply_rules(text: &str, rules: &[Rule]) -> String {
    let mut words: Vec<String> = text.split_whitespace().map(|s| s.to_owned()).collect();

    for rule in rules {
        if rule.from.is_empty() {
            continue;
        }

        let pattern_len = rule.from.len();
        let mut i = 0;
        while i + pattern_len <= words.len() {
            let matches = rule.from.iter().enumerate().all(|(j, pat)| {
                if pat == "*" {
                    true
                } else {
                    words[i + j] == *pat
                }
            });

            if matches {
                // Build replacement
                let replacement: Vec<String> = rule
                    .to
                    .iter()
                    .enumerate()
                    .map(|(j, rep)| {
                        if rep == "*" || rep == "$-" {
                            // Keep matched text
                            if j < pattern_len {
                                words[i + j].clone()
                            } else {
                                rep.clone()
                            }
                        } else {
                            rep.clone()
                        }
                    })
                    .collect();

                // Remove matched words and insert replacements
                words.splice(i..i + pattern_len, replacement);
                i += rule.to.len(); // Skip past replacement
            } else {
                i += 1;
            }
        }
    }

    words.join(" ")
}

/// Load a postmortem rules file.
///
/// Format: `from_pattern => to_replacement`
/// Lines starting with `#` or `;` are comments.
fn load_rules(path: &Path) -> Result<Vec<Rule>, TransformError> {
    let content = std::fs::read_to_string(path).map_err(|e| {
        TransformError::Transform(format!("Cannot read rules file '{}': {e}", path.display()))
    })?;

    let mut rules = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        // Try both => and ==> as separators
        let (from_str, to_str) = if let Some(idx) = line.find("==>") {
            (&line[..idx], &line[idx + 3..])
        } else if let Some(idx) = line.find("=>") {
            (&line[..idx], &line[idx + 2..])
        } else {
            continue;
        };

        let from: Vec<String> = from_str.split_whitespace().map(|s| s.to_owned()).collect();
        let to: Vec<String> = to_str.split_whitespace().map(|s| s.to_owned()).collect();

        if !from.is_empty() {
            rules.push(Rule { from, to });
        }
    }

    Ok(rules)
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::{
        ChatFile, DependentTier, Line, MainTier, Mor, MorTier, MorWord, PosCategory, Terminator,
        UserDefinedDependentTier, Utterance, UtteranceContent, Word,
    };

    #[test]
    fn apply_simple_rule() {
        let rules = vec![Rule {
            from: vec!["det|the".to_owned()],
            to: vec!["det|a".to_owned()],
        }];
        let result = apply_rules("det|the n|dog .", &rules);
        assert_eq!(result, "det|a n|dog .");
    }

    #[test]
    fn apply_wildcard_rule() {
        let rules = vec![Rule {
            from: vec!["det|the".to_owned(), "*".to_owned()],
            to: vec!["det|a".to_owned(), "$-".to_owned()],
        }];
        let result = apply_rules("det|the n|dog .", &rules);
        assert_eq!(result, "det|a n|dog .");
    }

    #[test]
    fn no_match_unchanged() {
        let rules = vec![Rule {
            from: vec!["det|a".to_owned()],
            to: vec!["det|the".to_owned()],
        }];
        let result = apply_rules("n|dog v|run .", &rules);
        assert_eq!(result, "n|dog v|run .");
    }

    #[test]
    fn postmortem_errors_on_mor_rewrite() {
        let mut utt = Utterance::new(MainTier::new(
            "CHI",
            vec![UtteranceContent::Word(Box::new(Word::simple("dog")))],
            Terminator::Period {
                span: talkbank_model::Span::DUMMY,
            },
        ));
        utt.dependent_tiers
            .push(DependentTier::Mor(MorTier::new_mor(
                vec![
                    Mor::new(MorWord::new(PosCategory::new("det"), "the")),
                    Mor::new(MorWord::new(PosCategory::new("n"), "dog")),
                ],
                talkbank_model::Terminator::Period {
                    span: talkbank_model::Span::DUMMY,
                },
            )));

        let mut file = ChatFile::new(vec![Line::Utterance(Box::new(utt))]);

        let cmd = PostmortemCommand {
            rules: vec![Rule {
                from: vec!["det|the".to_owned()],
                to: vec!["det|a".to_owned()],
            }],
            target_tier: "mor".to_owned(),
        };

        let err = cmd
            .transform(&mut file)
            .expect_err("typed %mor rewrite should error");
        assert!(err.to_string().contains("does not support degrading %mor"));
    }

    #[test]
    fn postmortem_rewrites_user_defined_target() {
        let mut utt = Utterance::new(MainTier::new(
            "CHI",
            vec![UtteranceContent::Word(Box::new(Word::simple("dog")))],
            Terminator::Period {
                span: talkbank_model::Span::DUMMY,
            },
        ));
        utt.dependent_tiers
            .push(DependentTier::UserDefined(UserDefinedDependentTier {
                label: talkbank_model::NonEmptyString::new("xmor").unwrap(),
                content: talkbank_model::NonEmptyString::new("det|the n|dog .").unwrap(),
                span: talkbank_model::Span::DUMMY,
            }));
        let mut file = ChatFile::new(vec![Line::Utterance(Box::new(utt))]);

        let cmd = PostmortemCommand {
            rules: vec![Rule {
                from: vec!["det|the".to_owned()],
                to: vec!["det|a".to_owned()],
            }],
            target_tier: "xmor".to_owned(),
        };

        cmd.transform(&mut file)
            .expect("user-defined tier rewrite should succeed");

        let Line::Utterance(utt) = &file.lines[0] else {
            panic!("expected utterance");
        };
        match &utt.dependent_tiers[0] {
            DependentTier::UserDefined(tier) => assert_eq!(tier.content.as_str(), "det|a n|dog ."),
            other => panic!("expected rewritten user-defined tier, got {other:?}"),
        }
    }
}
