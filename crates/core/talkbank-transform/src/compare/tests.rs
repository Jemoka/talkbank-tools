use std::collections::BTreeMap;

use super::engine::{conform_with_mapping, find_best_segment, is_punct_or_filler};
use super::*;
use talkbank_model::{ChatFile, ErrorCollector, WriteChat};
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

/// Build a minimal CHAT file with given utterance lines.
fn make_chat(utterances: &[(&str, &str)]) -> String {
    let mut lines = vec![
        "@UTF8".to_string(),
        "@Begin".to_string(),
        "@Languages:\teng".to_string(),
        "@Participants:\tCHI Target_Child, MOT Mother".to_string(),
        "@ID:\teng|test|CHI|3;|female|||Target_Child|||".to_string(),
        "@ID:\teng|test|MOT||female|||Mother|||".to_string(),
    ];
    for (speaker, text) in utterances {
        lines.push(format!("*{speaker}:\t{text}"));
    }
    lines.push("@End".to_string());
    lines.join("\n")
}

#[test]
fn identical_transcripts() {
    let parser = TreeSitterParser::new().unwrap();
    let chat = make_chat(&[("CHI", "hello world ."), ("MOT", "good morning .")]);
    let (main_file, _) = parse_lenient(&parser, &chat);
    let (gold_file, _) = parse_lenient(&parser, &chat);

    let result = compare(&main_file, &gold_file);
    assert_eq!(result.metrics.wer, 0.0);
    assert_eq!(result.metrics.accuracy, 1.0);
    assert_eq!(result.metrics.matches, 4);
    assert_eq!(result.metrics.insertions, 0);
    assert_eq!(result.metrics.deletions, 0);
    assert_eq!(result.metrics.total_gold_words, 4);
    assert_eq!(result.metrics.total_main_words, 4);
}

#[test]
fn single_substitution() {
    let parser = TreeSitterParser::new().unwrap();
    let main = make_chat(&[("CHI", "hello earth .")]);
    let gold = make_chat(&[("CHI", "hello world .")]);
    let (main_file, _) = parse_lenient(&parser, &main);
    let (gold_file, _) = parse_lenient(&parser, &gold);

    let result = compare(&main_file, &gold_file);
    // Compare aligns inside the best local window and does not surface
    // skipped main tokens outside that window as insertions.
    assert!(result.metrics.wer > 0.0);
    assert_eq!(result.metrics.matches, 1); // "hello" matches
    assert_eq!(result.metrics.insertions, 0); // skipped "earth"
    assert_eq!(result.metrics.deletions, 1); // "world"
}

#[test]
fn extra_word_in_main() {
    let parser = TreeSitterParser::new().unwrap();
    let main = make_chat(&[("CHI", "hello big world .")]);
    let gold = make_chat(&[("CHI", "hello world .")]);
    let (main_file, _) = parse_lenient(&parser, &main);
    let (gold_file, _) = parse_lenient(&parser, &gold);

    let result = compare(&main_file, &gold_file);
    assert_eq!(result.metrics.matches, 2); // "hello", "world"
    assert_eq!(result.metrics.insertions, 1); // "big"
    assert_eq!(result.metrics.deletions, 0);
    assert_eq!(result.metrics.total_gold_words, 2);
    assert_eq!(result.metrics.total_main_words, 3);
}

#[test]
fn missing_word_in_main() {
    let parser = TreeSitterParser::new().unwrap();
    let main = make_chat(&[("CHI", "hello .")]);
    let gold = make_chat(&[("CHI", "hello world .")]);
    let (main_file, _) = parse_lenient(&parser, &main);
    let (gold_file, _) = parse_lenient(&parser, &gold);

    let result = compare(&main_file, &gold_file);
    assert_eq!(result.metrics.matches, 1); // "hello"
    assert_eq!(result.metrics.insertions, 0);
    assert_eq!(result.metrics.deletions, 1); // "world"
    assert_eq!(result.metrics.total_gold_words, 2);
    assert_eq!(result.metrics.total_main_words, 1);
}

#[test]
fn empty_main() {
    let parser = TreeSitterParser::new().unwrap();
    // Main has an utterance but no content words (just terminator)
    let main = make_chat(&[("CHI", ".")]);
    let gold = make_chat(&[("CHI", "hello world .")]);
    let (main_file, _) = parse_lenient(&parser, &main);
    let (gold_file, _) = parse_lenient(&parser, &gold);

    let result = compare(&main_file, &gold_file);
    assert_eq!(result.metrics.matches, 0);
    assert_eq!(result.metrics.deletions, 2);
    assert_eq!(result.metrics.wer, 1.0);
}

#[test]
fn empty_gold() {
    let parser = TreeSitterParser::new().unwrap();
    let main = make_chat(&[("CHI", "hello world .")]);
    let gold = make_chat(&[("CHI", ".")]);
    let (main_file, _) = parse_lenient(&parser, &main);
    let (gold_file, _) = parse_lenient(&parser, &gold);

    let result = compare(&main_file, &gold_file);
    assert_eq!(result.metrics.matches, 0);
    assert_eq!(result.metrics.insertions, 0);
    assert_eq!(result.metrics.total_gold_words, 0);
    assert_eq!(result.metrics.wer, 0.0); // no gold words => wer=0
}

#[test]
fn case_insensitive_matching() {
    let parser = TreeSitterParser::new().unwrap();
    let main = make_chat(&[("CHI", "Hello World .")]);
    let gold = make_chat(&[("CHI", "hello world .")]);
    let (main_file, _) = parse_lenient(&parser, &main);
    let (gold_file, _) = parse_lenient(&parser, &gold);

    let result = compare(&main_file, &gold_file);
    assert_eq!(result.metrics.wer, 0.0);
    assert_eq!(result.metrics.matches, 2);
}

#[test]
fn conform_normalizes_contractions() {
    let parser = TreeSitterParser::new().unwrap();
    // "he's" should be expanded to "he is" by conform_words
    let main = make_chat(&[("CHI", "he's going .")]);
    let gold = make_chat(&[("CHI", "he is going .")]);
    let (main_file, _) = parse_lenient(&parser, &main);
    let (gold_file, _) = parse_lenient(&parser, &gold);

    let result = compare(&main_file, &gold_file);
    // After conform: main = ["he", "is", "going"], gold = ["he", "is", "going"]
    assert_eq!(result.metrics.wer, 0.0);
}

#[test]
fn multiple_utterances() {
    let parser = TreeSitterParser::new().unwrap();
    let main = make_chat(&[("CHI", "hello ."), ("MOT", "goodbye .")]);
    let gold = make_chat(&[("CHI", "hello ."), ("MOT", "goodbye .")]);
    let (main_file, _) = parse_lenient(&parser, &main);
    let (gold_file, _) = parse_lenient(&parser, &gold);

    let result = compare(&main_file, &gold_file);
    assert_eq!(result.metrics.wer, 0.0);
    assert_eq!(result.metrics.matches, 2);
    assert_eq!(result.main_utterances.len(), 2);
    assert_eq!(result.gold_utterances.len(), 2);
}

#[test]
fn xsrep_tier_content_serializes_through_write_chat() {
    let utt = UtteranceComparison {
        utterance_index: 0,
        speaker: "CHI".to_string(),
        tokens: vec![
            CompareToken {
                text: "hello".to_string(),
                pos: Some("INTJ".to_string()),
                status: CompareStatus::Match,
            },
            CompareToken {
                text: "big".to_string(),
                pos: Some("ADJ".to_string()),
                status: CompareStatus::ExtraMain,
            },
            CompareToken {
                text: "world".to_string(),
                pos: Some("NOUN".to_string()),
                status: CompareStatus::Match,
            },
            CompareToken {
                text: "today".to_string(),
                pos: Some("NOUN".to_string()),
                status: CompareStatus::ExtraGold,
            },
        ],
    };
    let xsrep = XsrepTierContent::try_from(&utt).expect("xsrep tier");
    assert_eq!(xsrep.to_chat_string(), "hello +big world -today");
}

#[test]
fn xsmor_tier_content_serializes_through_write_chat() {
    let utt = UtteranceComparison {
        utterance_index: 0,
        speaker: "CHI".to_string(),
        tokens: vec![
            CompareToken {
                text: "hello".to_string(),
                pos: Some("INTJ".to_string()),
                status: CompareStatus::Match,
            },
            CompareToken {
                text: "big".to_string(),
                pos: Some("ADJ".to_string()),
                status: CompareStatus::ExtraMain,
            },
            CompareToken {
                text: "today".to_string(),
                pos: None,
                status: CompareStatus::ExtraGold,
            },
        ],
    };
    let xsmor = XsmorTierContent::try_from(&utt).expect("xsmor tier");
    assert_eq!(xsmor.to_chat_string(), "INTJ +ADJ -?");
}

#[test]
fn xsmor_serializes_final_punctuation_as_surface_delimiter() {
    let utt = UtteranceComparison {
        utterance_index: 0,
        speaker: "CHI".to_string(),
        tokens: vec![
            CompareToken {
                text: "hello".to_string(),
                pos: Some("INTJ".to_string()),
                status: CompareStatus::Match,
            },
            CompareToken {
                text: ".".to_string(),
                pos: Some("PUNCT".to_string()),
                status: CompareStatus::Match,
            },
        ],
    };
    let xsmor = XsmorTierContent::try_from(&utt).expect("xsmor tier");
    assert_eq!(xsmor.to_chat_string(), "INTJ .");
}

#[test]
fn compare_metrics_csv_table_serializes_with_csv_writer() {
    let metrics = CompareMetrics {
        wer: 0.25,
        accuracy: 0.75,
        matches: 3,
        insertions: 1,
        deletions: 0,
        total_gold_words: 3,
        total_main_words: 4,
        pos_counts: BTreeMap::from([(
            "NOUN".to_string(),
            PosErrorCounts {
                matches: 2,
                insertions: 1,
                deletions: 0,
            },
        )]),
    };
    let csv = CompareMetricsCsvTable::from_metrics(&metrics)
        .expect("table")
        .to_csv_string()
        .expect("csv");
    assert!(csv.contains("wer,0.2500"));
    assert!(csv.contains("accuracy,0.7500"));
    assert!(csv.contains("matches,3"));
    assert!(csv.contains("insertions,1"));
    assert!(csv.contains("deletions,0"));
    assert!(csv.contains("NOUN:matches,2"));
    assert!(csv.contains("NOUN:insertions,1"));
}

#[test]
fn wer_computation_is_correct() {
    // 2 matches, 1 insertion, 1 deletion out of 3 gold words
    let metrics = CompareMetrics {
        wer: 0.0,      // will be computed
        accuracy: 0.0, // will be computed
        matches: 2,
        insertions: 1,
        deletions: 1,
        total_gold_words: 3, // matches + deletions
        total_main_words: 3, // matches + insertions
        pos_counts: BTreeMap::new(),
    };
    // WER = (ins + del) / total_gold = 2/3 ≈ 0.6667
    let expected_wer = 2.0 / 3.0;
    let _ = metrics;

    // Test via actual compare
    let parser = TreeSitterParser::new().unwrap();
    let main = make_chat(&[("CHI", "hello big world .")]);
    let gold = make_chat(&[("CHI", "hello world today .")]);
    let (main_file, _) = parse_lenient(&parser, &main);
    let (gold_file, _) = parse_lenient(&parser, &gold);

    let result = compare(&main_file, &gold_file);
    // main: hello, big, world
    // gold: hello, world, today
    // align: hello=match, big=extra_main, world=match, today=extra_gold
    assert_eq!(result.metrics.matches, 2);
    assert_eq!(result.metrics.insertions, 1);
    assert_eq!(result.metrics.deletions, 1);
    assert!((result.metrics.wer - expected_wer).abs() < 0.001);
}

#[test]
fn is_punct_or_filler_works() {
    assert!(is_punct_or_filler("."));
    assert!(is_punct_or_filler("?"));
    assert!(is_punct_or_filler("!"));
    assert!(is_punct_or_filler("+/."));
    assert!(is_punct_or_filler("um"));
    assert!(is_punct_or_filler("uh"));
    assert!(!is_punct_or_filler("hello"));
    assert!(!is_punct_or_filler("world"));
}

/// BA2-master commit 86230ef (2026-04-17, "fix part 2 of compare")
/// reordered the `_find_best_segment` tiebreakers so `align_matches`
/// (Levenshtein in-order matches) dominates `waste` (non-matching
/// tokens in the window). This avoids picking a window that has fewer
/// wasted tokens but represents cross-utterance fragments rather than
/// in-order in-utterance matches.
///
/// Constructed pair:
///   gold = [a, b, c]
///   main = [c, a, b, x, a, x, b, x, c]
///
/// Score-1.0 candidate windows:
///   [0..3] = {c,a,b}            waste=0  align_matches=2
///   [4..9] = {a, x, b, x, c}    waste=2  align_matches=3
///
/// Pre-86230ef order (waste primary): picks (0, 3).
/// Post-86230ef order (align_matches primary): picks (4, 9).
#[test]
fn find_best_segment_prefers_align_matches_over_waste() {
    let gold: Vec<String> = ["a", "b", "c"].iter().map(|s| s.to_string()).collect();
    let main: Vec<String> = ["c", "a", "b", "x", "a", "x", "b", "x", "c"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(
        find_best_segment(&gold, &main),
        (4, 9),
        "should pick the higher-align-match window even if it wastes more tokens"
    );
}

#[test]
fn conform_with_mapping_tracks_indices() {
    let words: Vec<String> = vec!["he's".to_string(), "going".to_string()];
    let (conformed, mapping) = conform_with_mapping(&words);
    // "he's" -> ["he", "is"], "going" -> ["going"]
    assert_eq!(conformed, vec!["he", "is", "going"]);
    assert_eq!(mapping, vec![0, 0, 1]);
}

#[test]
fn inject_comparison_adds_xsrep_tiers() {
    let parser = TreeSitterParser::new().unwrap();
    let main = make_chat(&[("CHI", "hello big world .")]);
    let gold = make_chat(&[("CHI", "hello world .")]);
    let (mut main_file, _) = parse_lenient(&parser, &main);
    let (gold_file, _) = parse_lenient(&parser, &gold);

    let result = compare(&main_file, &gold_file);
    inject_comparison(&mut main_file, &result.main_utterances).expect("inject comparison");

    // Find the utterance and check it has an %xsrep tier
    let serialized = main_file.to_chat_string();
    assert!(
        serialized.contains("%xsrep:"),
        "Output should contain %xsrep tier"
    );
    assert!(
        serialized.contains("+big"),
        "Should mark 'big' as extra_main"
    );
    assert!(
        serialized.contains("%xsmor:"),
        "Output should contain %xsmor tier"
    );
}

#[test]
fn clear_comparison_removes_compare_tiers() {
    let parser = TreeSitterParser::new().unwrap();
    let main = make_chat(&[("CHI", "hello world .")]);
    let gold = make_chat(&[("CHI", "hello world .")]);
    let (mut main_file, _) = parse_lenient(&parser, &main);
    let (gold_file, _) = parse_lenient(&parser, &gold);

    let result = compare(&main_file, &gold_file);
    inject_comparison(&mut main_file, &result.main_utterances).expect("inject comparison");

    // Verify xsrep was added
    let serialized = main_file.to_chat_string();
    assert!(serialized.contains("%xsrep:"));
    assert!(serialized.contains("%xsmor:"));

    // Clear and verify removal
    clear_comparison(&mut main_file);
    let serialized = main_file.to_chat_string();
    assert!(!serialized.contains("%xsrep:"));
    assert!(!serialized.contains("%xsmor:"));
}

#[test]
fn inject_comparison_idempotent() {
    let parser = TreeSitterParser::new().unwrap();
    let main = make_chat(&[("CHI", "hello big world .")]);
    let gold = make_chat(&[("CHI", "hello world .")]);
    let (mut main_file, _) = parse_lenient(&parser, &main);
    let (gold_file, _) = parse_lenient(&parser, &gold);

    let result = compare(&main_file, &gold_file);
    inject_comparison(&mut main_file, &result.main_utterances).expect("inject comparison");
    let first = main_file.to_chat_string();

    // Inject again — should produce the same output (replace, not duplicate)
    inject_comparison(&mut main_file, &result.main_utterances).expect("inject comparison");
    let second = main_file.to_chat_string();
    assert_eq!(first, second);
}

#[test]
fn format_metrics_csv_has_header() {
    let metrics = CompareMetrics {
        wer: 0.25,
        accuracy: 0.75,
        matches: 3,
        insertions: 1,
        deletions: 0,
        total_gold_words: 3,
        total_main_words: 4,
        pos_counts: BTreeMap::new(),
    };
    let csv = CompareMetricsCsvTable::from_metrics(&metrics)
        .expect("table")
        .to_csv_string()
        .expect("csv");
    assert!(csv.starts_with("metric,value\n"));
    assert!(csv.contains("wer,0.2500"));
}

#[test]
fn inject_comparison_rejects_empty_compare_tokens() {
    let parser = TreeSitterParser::new().unwrap();
    let main = make_chat(&[("CHI", "hello .")]);
    let (mut main_file, _) = parse_lenient(&parser, &main);

    let utterances = vec![UtteranceComparison {
        utterance_index: 0,
        speaker: "CHI".to_string(),
        tokens: vec![CompareToken {
            text: String::new(),
            pos: Some("INTJ".to_string()),
            status: CompareStatus::Match,
        }],
    }];

    let err =
        inject_comparison(&mut main_file, &utterances).expect_err("should reject empty token");
    assert!(err.to_string().contains("empty content"));
}

#[test]
fn compare_uses_mor_pos_for_xsmor_output() {
    let parser = TreeSitterParser::new().unwrap();
    let main = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI|||||Target_Child|||\n*CHI:\thello world .\n%mor:\tintj|hello noun|world .\n@End\n";
    let gold = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI|||||Target_Child|||\n*CHI:\thello world .\n@End\n";
    let (main_file, _) = parse_lenient(&parser, main);
    let (gold_file, _) = parse_lenient(&parser, gold);

    let result = compare(&main_file, &gold_file);
    assert_eq!(
        XsmorTierContent::try_from(&result.main_utterances[0])
            .expect("xsmor tier")
            .to_chat_string(),
        "INTJ NOUN"
    );
    assert_eq!(result.metrics.pos_counts["INTJ"].matches, 1);
    assert_eq!(result.metrics.pos_counts["NOUN"].matches, 1);
}

#[test]
fn compare_ignores_pos_punct_even_when_surface_is_not_punctuation() {
    let parser = TreeSitterParser::new().unwrap();
    let main = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI|||||Target_Child|||\n*CHI:\thello comma .\n%mor:\tintj|hello PUNCT|comma .\n@End\n";
    let gold = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI|||||Target_Child|||\n*CHI:\thello .\n@End\n";
    let (main_file, _) = parse_lenient(&parser, main);
    let (gold_file, _) = parse_lenient(&parser, gold);

    let result = compare(&main_file, &gold_file);
    assert_eq!(result.metrics.matches, 1);
    assert_eq!(result.metrics.insertions, 0);
    assert_eq!(result.metrics.deletions, 0);
    assert_eq!(result.metrics.wer, 0.0);
}

#[test]
fn compare_rotation_recovers_cyclic_window_matches() {
    let parser = TreeSitterParser::new().unwrap();
    let main = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI|||||Target_Child|||\n*CHI:\tworld hello .\n%mor:\tnoun|world intj|hello .\n@End\n";
    let gold = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI|||||Target_Child|||\n*CHI:\thello world .\n@End\n";
    let (main_file, _) = parse_lenient(&parser, main);
    let (gold_file, _) = parse_lenient(&parser, gold);

    let result = compare(&main_file, &gold_file);
    assert_eq!(result.metrics.matches, 2);
    assert_eq!(result.metrics.insertions, 0);
    assert_eq!(result.metrics.deletions, 0);
    assert_eq!(
        XsrepTierContent::try_from(&result.gold_utterances[0])
            .expect("xsrep tier")
            .to_chat_string(),
        "hello world ."
    );
}

#[test]
fn batchalign2_master_simple_gold_projection_shape() {
    let parser = TreeSitterParser::new().unwrap();
    let main = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI|||||Target_Child|||\n*CHI:\thello big world .\n%mor:\tintj|hello adj|big noun|world .\n@End\n";
    let gold = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI|||||Target_Child|||\n*CHI:\thello world today .\n@End\n";
    let (main_file, _) = parse_lenient(&parser, main);
    let (gold_file, _) = parse_lenient(&parser, gold);

    let result = compare(&main_file, &gold_file);
    assert_eq!(
        XsrepTierContent::try_from(&result.gold_utterances[0])
            .expect("xsrep tier")
            .to_chat_string(),
        "hello +big world -today ."
    );
    assert_eq!(
        XsmorTierContent::try_from(&result.gold_utterances[0])
            .expect("xsmor tier")
            .to_chat_string(),
        "INTJ +ADJ NOUN -? ."
    );
    assert_eq!(result.metrics.matches, 2);
    assert_eq!(result.metrics.insertions, 1);
    assert_eq!(result.metrics.deletions, 1);
    assert!((result.metrics.wer - (2.0 / 3.0)).abs() < 0.001);
    assert_eq!(result.metrics.pos_counts["ADJ"].insertions, 1);
    assert_eq!(result.metrics.pos_counts["?"].deletions, 1);
}

#[test]
fn batchalign2_master_windowed_alignment_ignores_skipped_prefix_tokens() {
    let parser = TreeSitterParser::new().unwrap();
    let main = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI|||||Target_Child|||\n*CHI:\tdog dog the dog .\n%mor:\tnoun|dog noun|dog det|the noun|dog .\n@End\n";
    let gold = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI|||||Target_Child|||\n*CHI:\tthe dog .\n@End\n";
    let (main_file, _) = parse_lenient(&parser, main);
    let (gold_file, _) = parse_lenient(&parser, gold);

    let result = compare(&main_file, &gold_file);
    assert_eq!(result.metrics.matches, 2);
    assert_eq!(result.metrics.insertions, 0);
    assert_eq!(result.metrics.deletions, 0);
    assert_eq!(result.metrics.wer, 0.0);
    assert_eq!(
        XsrepTierContent::try_from(&result.gold_utterances[0])
            .expect("xsrep tier")
            .to_chat_string(),
        "the dog ."
    );
    assert_eq!(
        XsmorTierContent::try_from(&result.gold_utterances[0])
            .expect("xsmor tier")
            .to_chat_string(),
        "DET NOUN ."
    );
}

#[test]
fn batchalign2_master_multi_utterance_compare_metrics() {
    let parser = TreeSitterParser::new().unwrap();
    let main = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI|||||Target_Child|||\n*CHI:\tone fish two fish .\n%mor:\tnum|one noun|fish num|two noun|fish .\n*CHI:\tred fish blue fish .\n%mor:\tadj|red noun|fish adj|blue noun|fish .\n@End\n";
    let gold = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI|||||Target_Child|||\n*CHI:\tone fish fish .\n*CHI:\tred fish green fish .\n@End\n";
    let (main_file, _) = parse_lenient(&parser, main);
    let (gold_file, _) = parse_lenient(&parser, gold);

    let result = compare(&main_file, &gold_file);
    assert_eq!(
        XsrepTierContent::try_from(&result.gold_utterances[0])
            .expect("xsrep tier")
            .to_chat_string(),
        "one fish +two fish ."
    );
    assert_eq!(
        XsmorTierContent::try_from(&result.gold_utterances[0])
            .expect("xsmor tier")
            .to_chat_string(),
        "NUM NOUN +NUM NOUN ."
    );
    assert_eq!(
        XsrepTierContent::try_from(&result.gold_utterances[1])
            .expect("xsrep tier")
            .to_chat_string(),
        "red fish -green +blue fish ."
    );
    assert_eq!(
        XsmorTierContent::try_from(&result.gold_utterances[1])
            .expect("xsmor tier")
            .to_chat_string(),
        "ADJ NOUN -? +ADJ NOUN ."
    );
    assert_eq!(result.metrics.matches, 6);
    assert_eq!(result.metrics.insertions, 2);
    assert_eq!(result.metrics.deletions, 1);
    assert!((result.metrics.wer - (3.0 / 7.0)).abs() < 0.001);
}

#[test]
fn gold_anchored_projection_attaches_diff_to_gold_transcript() {
    let parser = TreeSitterParser::new().unwrap();
    let main = make_chat(&[("CHI", "hello big world .")]);
    let gold = make_chat(&[("CHI", "hello world today .")]);
    let (main_file, _) = parse_lenient(&parser, &main);
    let (mut gold_file, _) = parse_lenient(&parser, &gold);

    let result = compare(&main_file, &gold_file);
    inject_comparison(&mut gold_file, &result.gold_utterances).expect("inject comparison");

    let serialized = gold_file.to_chat_string();
    assert!(serialized.contains("*CHI:\thello world today ."));
    assert!(serialized.contains("%xsrep:\thello +big world -today"));
    assert!(serialized.contains("%xsmor:"));
}
