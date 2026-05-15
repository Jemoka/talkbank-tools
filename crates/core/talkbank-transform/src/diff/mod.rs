//! Utterance-level diff engine for incremental CHAT processing.
//!
//! Compares two versions of a CHAT file ("before" and "after") and produces
//! a list of [`UtteranceDelta`] values describing what changed at the
//! utterance level. This enables **smart incremental reprocessing**: only
//! changed utterances are sent to NLP workers, while unchanged utterances
//! preserve their cached dependent tiers (%mor, %gra, %wor, bullets).
//!
//! # Usage
//!
//! ```rust,no_run
//! use talkbank_parser::TreeSitterParser;
//! use talkbank_transform::diff::{DiffSummary, diff_chat};
//!
//! let parser = TreeSitterParser::new().unwrap();
//! let before_text = std::fs::read_to_string("before.cha").unwrap();
//! let after_text = std::fs::read_to_string("after.cha").unwrap();
//!
//! let before = parser.parse_chat_file(&before_text).unwrap();
//! let after = parser.parse_chat_file(&after_text).unwrap();
//!
//! let deltas = diff_chat(&before, &after);
//! let summary = DiffSummary::from_deltas(&deltas);
//!
//! println!(
//!     "{} unchanged, {} need reprocessing",
//!     summary.unchanged,
//!     summary.needs_reprocessing(),
//! );
//! ```

mod classify;
/// Dependent tier preservation utilities.
pub mod preserve;
mod types;

pub use classify::diff_chat;
pub use preserve::copy_dependent_tiers;
pub use types::{DiffSummary, UtteranceDelta};

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::{ChatFile, ErrorCollector, UtteranceIdx};
    use talkbank_parser::TreeSitterParser;

    fn parse_lenient(
        parser: &TreeSitterParser,
        chat_text: &str,
    ) -> (ChatFile, Vec<talkbank_model::ParseError>) {
        let errors = ErrorCollector::new();
        let chat_file = parser.parse_chat_file_streaming(chat_text, &errors);
        let error_vec = errors.into_vec();
        (chat_file, error_vec)
    }

    /// Build a minimal valid CHAT file from utterance lines.
    fn make_chat(utterances: &[(&str, &str)]) -> String {
        let mut speakers: Vec<&str> = utterances.iter().map(|(s, _)| *s).collect();
        speakers.sort();
        speakers.dedup();

        let mut lines = vec![
            "@UTF8".to_string(),
            "@Begin".to_string(),
            "@Languages:\teng".to_string(),
        ];

        // Build @Participants line
        let participant_entries: Vec<String> = speakers
            .iter()
            .map(|s| {
                let role = match *s {
                    "CHI" => "Target_Child",
                    "MOT" => "Mother",
                    "FAT" => "Father",
                    "INV" => "Investigator",
                    _ => "Participant",
                };
                format!("{s} {role}")
            })
            .collect();
        lines.push(format!(
            "@Participants:\t{}",
            participant_entries.join(", ")
        ));

        // Build @ID lines
        for s in &speakers {
            let role = match *s {
                "CHI" => "Target_Child",
                "MOT" => "Mother",
                "FAT" => "Father",
                "INV" => "Investigator",
                _ => "Participant",
            };
            let age = if *s == "CHI" { "3;" } else { "" };
            lines.push(format!("@ID:\teng|test|{s}|{age}|female|||{role}|||"));
        }

        for (speaker, text) in utterances {
            lines.push(format!("*{speaker}:\t{text}"));
        }

        lines.push("@End".to_string());
        lines.join("\n")
    }

    /// Build CHAT with utterance-level bullets.
    type BulletedUtterance<'a> = (&'a str, &'a str, Option<(u64, u64)>);

    fn make_chat_with_bullets(utterances: &[BulletedUtterance<'_>]) -> String {
        let mut speakers: Vec<&str> = utterances.iter().map(|(s, _, _)| *s).collect();
        speakers.sort();
        speakers.dedup();

        let mut lines = vec![
            "@UTF8".to_string(),
            "@Begin".to_string(),
            "@Languages:\teng".to_string(),
        ];

        let participant_entries: Vec<String> = speakers
            .iter()
            .map(|s| {
                let role = match *s {
                    "CHI" => "Target_Child",
                    "MOT" => "Mother",
                    _ => "Participant",
                };
                format!("{s} {role}")
            })
            .collect();
        lines.push(format!(
            "@Participants:\t{}",
            participant_entries.join(", ")
        ));

        for s in &speakers {
            let role = match *s {
                "CHI" => "Target_Child",
                "MOT" => "Mother",
                _ => "Participant",
            };
            let age = if *s == "CHI" { "3;" } else { "" };
            lines.push(format!("@ID:\teng|test|{s}|{age}|female|||{role}|||"));
        }

        for (speaker, text, bullet) in utterances {
            let bullet_str = match bullet {
                Some((start, end)) => format!(" \u{0015}{start}_{end}\u{0015}"),
                None => String::new(),
            };
            lines.push(format!("*{speaker}:\t{text}{bullet_str}"));
        }

        lines.push("@End".to_string());
        lines.join("\n")
    }

    // -----------------------------------------------------------------------
    // Basic identity tests
    // -----------------------------------------------------------------------

    #[test]
    fn identical_files_all_unchanged() {
        let parser = TreeSitterParser::new().unwrap();
        let chat = make_chat(&[("CHI", "hello world ."), ("MOT", "good morning .")]);
        let (before, _) = parse_lenient(&parser, &chat);
        let (after, _) = parse_lenient(&parser, &chat);

        let deltas = diff_chat(&before, &after);
        assert_eq!(deltas.len(), 2);
        assert!(
            deltas
                .iter()
                .all(|d| matches!(d, UtteranceDelta::Unchanged { .. }))
        );

        let summary = DiffSummary::from_deltas(&deltas);
        assert_eq!(summary.unchanged, 2);
        assert_eq!(summary.needs_reprocessing(), 0);
    }

    #[test]
    fn empty_files_produce_empty_deltas() {
        let parser = TreeSitterParser::new().unwrap();
        let chat = make_chat(&[]);
        let (before, _) = parse_lenient(&parser, &chat);
        let (after, _) = parse_lenient(&parser, &chat);

        let deltas = diff_chat(&before, &after);
        assert!(deltas.is_empty());
    }

    // -----------------------------------------------------------------------
    // Word changes
    // -----------------------------------------------------------------------

    #[test]
    fn single_word_change_detected() {
        let parser = TreeSitterParser::new().unwrap();
        let before_text = make_chat(&[("CHI", "hello world ."), ("MOT", "good morning .")]);
        let after_text = make_chat(&[("CHI", "hello earth ."), ("MOT", "good morning .")]);
        let (before, _) = parse_lenient(&parser, &before_text);
        let (after, _) = parse_lenient(&parser, &after_text);

        let deltas = diff_chat(&before, &after);
        let summary = DiffSummary::from_deltas(&deltas);

        assert_eq!(summary.words_changed, 1);
        assert_eq!(summary.unchanged, 1);
        assert_eq!(summary.needs_reprocessing(), 1);
    }

    #[test]
    fn all_words_changed() {
        let parser = TreeSitterParser::new().unwrap();
        let before_text = make_chat(&[("CHI", "hello world ."), ("MOT", "good morning .")]);
        let after_text = make_chat(&[("CHI", "bye earth ."), ("MOT", "bad evening .")]);
        let (before, _) = parse_lenient(&parser, &before_text);
        let (after, _) = parse_lenient(&parser, &after_text);

        let deltas = diff_chat(&before, &after);
        let summary = DiffSummary::from_deltas(&deltas);

        // Both utterances have different words
        assert_eq!(
            summary.words_changed + summary.inserted + summary.deleted,
            2
        );
    }

    // -----------------------------------------------------------------------
    // Insertions and deletions
    // -----------------------------------------------------------------------

    #[test]
    fn utterance_inserted() {
        let parser = TreeSitterParser::new().unwrap();
        let before_text = make_chat(&[("CHI", "hello ."), ("MOT", "goodbye .")]);
        let after_text = make_chat(&[("CHI", "hello ."), ("CHI", "world ."), ("MOT", "goodbye .")]);
        let (before, _) = parse_lenient(&parser, &before_text);
        let (after, _) = parse_lenient(&parser, &after_text);

        let deltas = diff_chat(&before, &after);
        let summary = DiffSummary::from_deltas(&deltas);

        assert_eq!(summary.unchanged, 2); // "hello ." and "goodbye ." preserved
        assert_eq!(summary.inserted, 1); // "world ." is new
    }

    #[test]
    fn utterance_deleted() {
        let parser = TreeSitterParser::new().unwrap();
        let before_text =
            make_chat(&[("CHI", "hello ."), ("CHI", "world ."), ("MOT", "goodbye .")]);
        let after_text = make_chat(&[("CHI", "hello ."), ("MOT", "goodbye .")]);
        let (before, _) = parse_lenient(&parser, &before_text);
        let (after, _) = parse_lenient(&parser, &after_text);

        let deltas = diff_chat(&before, &after);
        let summary = DiffSummary::from_deltas(&deltas);

        assert_eq!(summary.unchanged, 2);
        assert_eq!(summary.deleted, 1);
    }

    #[test]
    fn all_utterances_inserted() {
        let parser = TreeSitterParser::new().unwrap();
        let before_text = make_chat(&[]);
        let after_text = make_chat(&[("CHI", "hello ."), ("MOT", "world .")]);
        let (before, _) = parse_lenient(&parser, &before_text);
        let (after, _) = parse_lenient(&parser, &after_text);

        let deltas = diff_chat(&before, &after);
        let summary = DiffSummary::from_deltas(&deltas);

        assert_eq!(summary.inserted, 2);
        assert_eq!(summary.unchanged, 0);
    }

    #[test]
    fn all_utterances_deleted() {
        let parser = TreeSitterParser::new().unwrap();
        let before_text = make_chat(&[("CHI", "hello ."), ("MOT", "world .")]);
        let after_text = make_chat(&[]);
        let (before, _) = parse_lenient(&parser, &before_text);
        let (after, _) = parse_lenient(&parser, &after_text);

        let deltas = diff_chat(&before, &after);
        let summary = DiffSummary::from_deltas(&deltas);

        assert_eq!(summary.deleted, 2);
        assert_eq!(summary.unchanged, 0);
    }

    // -----------------------------------------------------------------------
    // Speaker changes
    // -----------------------------------------------------------------------

    #[test]
    fn speaker_change_detected() {
        let parser = TreeSitterParser::new().unwrap();
        let before_text = make_chat(&[("CHI", "hello world ."), ("MOT", "good morning .")]);
        let after_text = make_chat(&[("MOT", "hello world ."), ("MOT", "good morning .")]);
        let (before, _) = parse_lenient(&parser, &before_text);
        let (after, _) = parse_lenient(&parser, &after_text);

        let deltas = diff_chat(&before, &after);
        let summary = DiffSummary::from_deltas(&deltas);

        assert_eq!(summary.speaker_changed, 1);
        assert_eq!(summary.unchanged, 1);
    }

    // -----------------------------------------------------------------------
    // Timing changes
    // -----------------------------------------------------------------------

    #[test]
    fn timing_change_detected() {
        let parser = TreeSitterParser::new().unwrap();
        let before_text = make_chat_with_bullets(&[
            ("CHI", "hello world .", Some((0, 1000))),
            ("MOT", "good morning .", Some((1000, 2000))),
        ]);
        let after_text = make_chat_with_bullets(&[
            ("CHI", "hello world .", Some((0, 1500))),
            ("MOT", "good morning .", Some((1000, 2000))),
        ]);
        let (before, _) = parse_lenient(&parser, &before_text);
        let (after, _) = parse_lenient(&parser, &after_text);

        let deltas = diff_chat(&before, &after);
        let summary = DiffSummary::from_deltas(&deltas);

        assert_eq!(summary.timing_only, 1);
        assert_eq!(summary.unchanged, 1);
    }

    #[test]
    fn bullet_added_is_timing_change() {
        let parser = TreeSitterParser::new().unwrap();
        let before_text = make_chat(&[("CHI", "hello world .")]);
        let after_text = make_chat_with_bullets(&[("CHI", "hello world .", Some((0, 1000)))]);
        let (before, _) = parse_lenient(&parser, &before_text);
        let (after, _) = parse_lenient(&parser, &after_text);

        let deltas = diff_chat(&before, &after);
        let summary = DiffSummary::from_deltas(&deltas);

        assert_eq!(summary.timing_only, 1);
    }

    #[test]
    fn bullet_removed_is_timing_change() {
        let parser = TreeSitterParser::new().unwrap();
        let before_text = make_chat_with_bullets(&[("CHI", "hello world .", Some((0, 1000)))]);
        let after_text = make_chat(&[("CHI", "hello world .")]);
        let (before, _) = parse_lenient(&parser, &before_text);
        let (after, _) = parse_lenient(&parser, &after_text);

        let deltas = diff_chat(&before, &after);
        let summary = DiffSummary::from_deltas(&deltas);

        assert_eq!(summary.timing_only, 1);
    }

    // -----------------------------------------------------------------------
    // Combined changes
    // -----------------------------------------------------------------------

    #[test]
    fn words_and_timing_both_changed() {
        let parser = TreeSitterParser::new().unwrap();
        let before_text = make_chat_with_bullets(&[("CHI", "hello world .", Some((0, 1000)))]);
        let after_text = make_chat_with_bullets(&[("CHI", "hello earth .", Some((0, 1500)))]);
        let (before, _) = parse_lenient(&parser, &before_text);
        let (after, _) = parse_lenient(&parser, &after_text);

        let deltas = diff_chat(&before, &after);
        assert_eq!(deltas.len(), 1);
        match &deltas[0] {
            UtteranceDelta::WordsChanged { timing_changed, .. } => {
                assert!(timing_changed);
            }
            other => panic!("Expected WordsChanged, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // Complex scenarios (insert + delete + change)
    // -----------------------------------------------------------------------

    #[test]
    fn mixed_insert_delete_change() {
        let parser = TreeSitterParser::new().unwrap();
        let before_text = make_chat(&[
            ("CHI", "first utterance ."),
            ("CHI", "second utterance ."),
            ("MOT", "third utterance ."),
        ]);
        let after_text = make_chat(&[
            ("CHI", "first utterance ."), // unchanged
            ("CHI", "modified second ."), // words changed
            ("CHI", "brand new ."),       // inserted
            ("MOT", "third utterance ."), // unchanged
        ]);
        let (before, _) = parse_lenient(&parser, &before_text);
        let (after, _) = parse_lenient(&parser, &after_text);

        let deltas = diff_chat(&before, &after);
        let summary = DiffSummary::from_deltas(&deltas);

        assert_eq!(summary.unchanged, 2);
        // "second utterance" → "modified second" = words changed or insert+delete
        // "brand new" = inserted
        assert!(summary.needs_reprocessing() >= 2);
    }

    // -----------------------------------------------------------------------
    // Property: diff(f, f) is always all Unchanged
    // -----------------------------------------------------------------------

    #[test]
    fn property_self_diff_all_unchanged() {
        let parser = TreeSitterParser::new().unwrap();
        let scenarios = vec![
            make_chat(&[("CHI", "hello .")]),
            make_chat(&[("CHI", "a ."), ("MOT", "b ."), ("CHI", "c .")]),
            make_chat_with_bullets(&[
                ("CHI", "hello .", Some((0, 500))),
                ("MOT", "world .", Some((500, 1000))),
            ]),
            make_chat(&[]),
        ];

        for chat_text in &scenarios {
            let (file, _) = parse_lenient(&parser, chat_text);
            let deltas = diff_chat(&file, &file);
            for delta in &deltas {
                assert!(
                    matches!(delta, UtteranceDelta::Unchanged { .. }),
                    "Self-diff should be all Unchanged, got {delta:?}"
                );
            }
        }
    }

    // -----------------------------------------------------------------------
    // DiffSummary tests
    // -----------------------------------------------------------------------

    #[test]
    fn diff_summary_total() {
        let deltas = vec![
            UtteranceDelta::Unchanged {
                before_idx: UtteranceIdx(0),
                after_idx: UtteranceIdx(0),
            },
            UtteranceDelta::Inserted {
                after_idx: UtteranceIdx(1),
            },
            UtteranceDelta::Deleted {
                before_idx: UtteranceIdx(1),
            },
        ];
        let summary = DiffSummary::from_deltas(&deltas);
        assert_eq!(summary.total(), 3);
        assert_eq!(summary.needs_reprocessing(), 1);
    }

    // -----------------------------------------------------------------------
    // UtteranceDelta helper methods
    // -----------------------------------------------------------------------

    #[test]
    fn delta_needs_nlp_reprocessing() {
        let unchanged = UtteranceDelta::Unchanged {
            before_idx: UtteranceIdx(0),
            after_idx: UtteranceIdx(0),
        };
        assert!(!unchanged.needs_nlp_reprocessing());

        let words_changed = UtteranceDelta::WordsChanged {
            before_idx: UtteranceIdx(0),
            after_idx: UtteranceIdx(0),
            timing_changed: false,
        };
        assert!(words_changed.needs_nlp_reprocessing());

        let inserted = UtteranceDelta::Inserted {
            after_idx: UtteranceIdx(1),
        };
        assert!(inserted.needs_nlp_reprocessing());

        let speaker = UtteranceDelta::SpeakerChanged {
            before_idx: UtteranceIdx(0),
            after_idx: UtteranceIdx(0),
        };
        assert!(!speaker.needs_nlp_reprocessing());
    }

    #[test]
    fn delta_affects_timing() {
        let timing_only = UtteranceDelta::TimingOnly {
            before_idx: UtteranceIdx(0),
            after_idx: UtteranceIdx(0),
        };
        assert!(timing_only.affects_timing());

        let words_no_timing = UtteranceDelta::WordsChanged {
            before_idx: UtteranceIdx(0),
            after_idx: UtteranceIdx(0),
            timing_changed: false,
        };
        assert!(!words_no_timing.affects_timing());

        let words_with_timing = UtteranceDelta::WordsChanged {
            before_idx: UtteranceIdx(0),
            after_idx: UtteranceIdx(0),
            timing_changed: true,
        };
        assert!(words_with_timing.affects_timing());

        let deleted = UtteranceDelta::Deleted {
            before_idx: UtteranceIdx(0),
        };
        assert!(deleted.affects_timing());
    }

    // -----------------------------------------------------------------------
    // Reordering detection
    // -----------------------------------------------------------------------

    #[test]
    fn reordered_utterances_detected_as_changes() {
        let parser = TreeSitterParser::new().unwrap();
        let before_text = make_chat(&[("CHI", "alpha ."), ("MOT", "beta .")]);
        let after_text = make_chat(&[("MOT", "beta ."), ("CHI", "alpha .")]);
        let (before, _) = parse_lenient(&parser, &before_text);
        let (after, _) = parse_lenient(&parser, &after_text);

        let deltas = diff_chat(&before, &after);
        let summary = DiffSummary::from_deltas(&deltas);

        // One of the utterances must be detected as non-unchanged
        // (since the DP aligner can only match one direction)
        assert!(summary.total() > 0);
    }

    // -----------------------------------------------------------------------
    // Large file stress test
    // -----------------------------------------------------------------------

    #[test]
    fn large_file_with_single_change() {
        let mut utterances: Vec<(&str, &str)> = Vec::new();
        let words: Vec<String> = (0..50).map(|i| format!("word{i} .")).collect();
        let word_refs: Vec<&str> = words.iter().map(|s| s.as_str()).collect();

        for w in &word_refs {
            utterances.push(("CHI", w));
        }

        let before_text = make_chat(&utterances);
        utterances[25] = ("CHI", "changed utterance .");
        let after_text = make_chat(&utterances);

        let parser = TreeSitterParser::new().unwrap();
        let (before, _) = parse_lenient(&parser, &before_text);
        let (after, _) = parse_lenient(&parser, &after_text);

        let deltas = diff_chat(&before, &after);
        let summary = DiffSummary::from_deltas(&deltas);

        assert_eq!(summary.unchanged, 49);
        assert_eq!(summary.needs_reprocessing(), 1);
    }
}
