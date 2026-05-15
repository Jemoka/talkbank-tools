//! GEM — Extract Gem Segments.
//!
//! Extracts utterances and their dependent tiers that fall within `@Bg`/`@Eg`
//! gem boundaries, producing a new CHAT file containing only the gem-scoped
//! content. Headers and metadata are preserved from the original file.
//!
//! # CLAN Manual
//!
//! See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409206)
//! for the original GEM command specification.
//!
//! # CLAN Equivalence
//!
//! | CLAN command                      | Rust equivalent                              |
//! |-----------------------------------|----------------------------------------------|
//! | `gem file.cha`                    | `chatter clan gem file.cha`                  |
//! | `gem +g"story" file.cha`          | `chatter clan gem --gem story file.cha`       |
//!
//! # Differences from CLAN
//!
//! - Gem boundary detection operates on parsed `Header` variants from the AST
//!   rather than raw text line matching for `@BG:`/`@EG:`.
//! - Handles both `@Bg:`/`@Eg:` (mixed case) and `@BG:`/`@EG:` (uppercase).
//! - Without `--gem` filter, extracts all gem segments. With `--gem`, extracts
//!   only matching labels.

use talkbank_model::{ChatFile, Header, Line};

use crate::framework::{TransformCommand, TransformError};

/// GEM transform command.
///
/// Filters a CHAT file to retain only utterances within `@Bg`/`@Eg` gem
/// boundaries. The gem boundary headers themselves are preserved in the output.
#[derive(Debug, Clone, Default)]
pub struct GemCommand {
    /// Optional gem labels to extract. If empty, extract all gem segments.
    pub labels: Vec<String>,
}

impl GemCommand {
    /// Create a new GEM command filtering for specific labels.
    pub fn new(labels: Vec<String>) -> Self {
        Self { labels }
    }

    /// Check whether a gem label matches the filter (or filter is empty = match all).
    fn matches_label(&self, label: &str) -> bool {
        if self.labels.is_empty() {
            return true;
        }
        self.labels.iter().any(|l| l.eq_ignore_ascii_case(label))
    }
}

impl TransformCommand for GemCommand {
    type Config = ();

    fn transform(&self, chat_file: &mut ChatFile) -> Result<(), TransformError> {
        let mut active_gems: Vec<String> = Vec::new();
        let mut keep = Vec::new();

        for line in chat_file.lines.iter() {
            match line {
                Line::Header { header, .. } => {
                    match header.as_ref() {
                        Header::BeginGem { label } => {
                            let label_str = label
                                .as_ref()
                                .map(|l| l.as_str().to_owned())
                                .unwrap_or_default();
                            if self.matches_label(&label_str) {
                                active_gems.push(label_str);
                                keep.push(true); // Keep the @Bg header
                            } else {
                                keep.push(false);
                            }
                        }
                        Header::EndGem { label } => {
                            let label_str = label
                                .as_ref()
                                .map(|l| l.as_str().to_owned())
                                .unwrap_or_default();
                            if let Some(pos) = active_gems
                                .iter()
                                .rposition(|g| g.eq_ignore_ascii_case(&label_str))
                            {
                                active_gems.remove(pos);
                                keep.push(true); // Keep the @Eg header
                            } else {
                                keep.push(false);
                            }
                        }
                        // Keep all non-gem headers (metadata, participants, etc.)
                        _ => keep.push(true),
                    }
                }
                Line::Utterance(_) => {
                    keep.push(!active_gems.is_empty());
                }
            }
        }

        // Filter lines in place
        let mut idx = 0;
        chat_file.lines.retain(|_| {
            let k = keep.get(idx).copied().unwrap_or(false);
            idx += 1;
            k
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::Span;
    use talkbank_model::{GemLabel, MainTier, Terminator, Utterance, UtteranceContent, Word};

    fn utt_line(speaker: &str, words: &[&str]) -> Line {
        let content: Vec<UtteranceContent> = words
            .iter()
            .map(|w| UtteranceContent::Word(Box::new(Word::simple(*w))))
            .collect();
        let main = MainTier::new(speaker, content, Terminator::Period { span: Span::DUMMY });
        Line::utterance(Utterance::new(main))
    }

    fn bg_line(label: &str) -> Line {
        Line::header(Header::BeginGem {
            label: Some(GemLabel::new(label)),
        })
    }

    fn eg_line(label: &str) -> Line {
        Line::header(Header::EndGem {
            label: Some(GemLabel::new(label)),
        })
    }

    #[test]
    fn gem_extracts_scoped_utterances() {
        let cmd = GemCommand::default();
        let mut chat = ChatFile::new(vec![
            utt_line("CHI", &["before"]),
            bg_line("Story"),
            utt_line("CHI", &["inside"]),
            eg_line("Story"),
            utt_line("CHI", &["after"]),
        ]);

        cmd.transform(&mut chat).unwrap();

        // Should keep: @Bg header, "inside" utterance, @Eg header
        // Plus "before" and "after" should be removed
        let utterances: Vec<_> = chat
            .lines
            .iter()
            .filter_map(|l| match l {
                Line::Utterance(u) => Some(u.main.speaker.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(utterances.len(), 1);
    }

    #[test]
    fn gem_filters_by_label() {
        let cmd = GemCommand::new(vec!["Story".to_owned()]);
        let mut chat = ChatFile::new(vec![
            bg_line("Story"),
            utt_line("CHI", &["story"]),
            eg_line("Story"),
            bg_line("Play"),
            utt_line("CHI", &["play"]),
            eg_line("Play"),
        ]);

        cmd.transform(&mut chat).unwrap();

        let utterances: Vec<_> = chat
            .lines
            .iter()
            .filter_map(|l| match l {
                Line::Utterance(_) => Some(()),
                _ => None,
            })
            .collect();
        assert_eq!(utterances.len(), 1); // Only the "story" utterance
    }

    #[test]
    fn gem_empty_file_no_error() {
        let cmd = GemCommand::default();
        let mut chat = ChatFile::new(vec![]);
        assert!(cmd.transform(&mut chat).is_ok());
    }
}
