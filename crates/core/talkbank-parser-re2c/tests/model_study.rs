//! Study what TreeSitterParser produces for specific inputs.
//! This is research, not a test suite — run with --nocapture to see output.
//!
//! cargo test -p talkbank-parser-re2c --test model_study -- --nocapture --ignored

mod fixture_utils;

use talkbank_model::{ChatParser, SemanticEq};
use talkbank_parser::TreeSitterParser;

fn ts() -> TreeSitterParser {
    TreeSitterParser::new().expect("grammar loads")
}

#[test]
#[ignore]
fn study_simple_file() {
    let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child, MOT Mother\n@ID:\teng|corpus|CHI|3;00.||||Child|||\n*CHI:\thello world .\n%mor:\tn|hello n|world .\n%gra:\t1|2|SUBJ 2|0|ROOT .\n@End\n";
    let file = ts().parse_chat_file(input).unwrap();
    eprintln!("{}", serde_json::to_string_pretty(&file).unwrap());
}

#[test]
#[ignore]
fn study_main_tier_simple() {
    let mt = ts().parse_main_tier("*CHI:\thello world .\n").unwrap();
    eprintln!("{}", serde_json::to_string_pretty(&mt).unwrap());
}

#[test]
#[ignore]
fn study_main_tier_compound() {
    let mt = ts().parse_main_tier("*CHI:\tI want ice+cream .\n").unwrap();
    eprintln!("{}", serde_json::to_string_pretty(&mt).unwrap());
}

#[test]
#[ignore]
fn study_main_tier_annotations() {
    let mt = ts().parse_main_tier("*CHI:\tthe the [/] dog .\n").unwrap();
    eprintln!("{}", serde_json::to_string_pretty(&mt).unwrap());
}

#[test]
#[ignore]
fn study_word_simple() {
    let w = ts().parse_word("hello").unwrap();
    eprintln!("{}", serde_json::to_string_pretty(&w).unwrap());
}

// ═══════════════════════════════════════════════════════════════
// Word equivalence tests (not ignored — these run in CI)
// ═══════════════════════════════════════════════════════════════

#[test]
fn word_equivalence_simple() {
    let ts_word = ts().parse_word("hello").unwrap();
    let re2c_word = re2c_word("hello");
    assert!(
        ts_word.semantic_eq(&re2c_word),
        "simple word mismatch:\n  ts:   {}\n  re2c: {}",
        serde_json::to_string(&ts_word).unwrap(),
        serde_json::to_string(&re2c_word).unwrap(),
    );
}

#[test]
fn word_equivalence_compound() {
    let ts_word = ts().parse_word("ice+cream").unwrap();
    let re2c_word = re2c_word("ice+cream");
    assert!(
        ts_word.semantic_eq(&re2c_word),
        "compound word mismatch:\n  ts:   {}\n  re2c: {}",
        serde_json::to_string(&ts_word).unwrap(),
        serde_json::to_string(&re2c_word).unwrap(),
    );
}

#[test]
fn word_equivalence_lengthening() {
    let ts_word = ts().parse_word("no::").unwrap();
    let re2c_word = re2c_word("no::");
    assert!(
        ts_word.semantic_eq(&re2c_word),
        "lengthening mismatch:\n  ts:   {}\n  re2c: {}",
        serde_json::to_string(&ts_word).unwrap(),
        serde_json::to_string(&re2c_word).unwrap(),
    );
}

#[test]
fn word_equivalence_form_marker() {
    let ts_word = ts().parse_word("mama@f").unwrap();
    let re2c_word = re2c_word("mama@f");
    assert!(
        ts_word.semantic_eq(&re2c_word),
        "form marker mismatch:\n  ts:   {}\n  re2c: {}",
        serde_json::to_string(&ts_word).unwrap(),
        serde_json::to_string(&re2c_word).unwrap(),
    );
}

#[test]
fn word_equivalence_shortening() {
    let ts_word = ts().parse_word("(be)cause").unwrap();
    let re2c_word = re2c_word("(be)cause");
    assert!(
        ts_word.semantic_eq(&re2c_word),
        "shortening mismatch:\n  ts:   {}\n  re2c: {}",
        serde_json::to_string(&ts_word).unwrap(),
        serde_json::to_string(&re2c_word).unwrap(),
    );
}

#[test]
fn word_equivalence_filler() {
    let ts_word = ts().parse_word("&-um").unwrap();
    let re2c_word = re2c_word("&-um");
    assert!(
        ts_word.semantic_eq(&re2c_word),
        "filler mismatch:\n  ts:   {}\n  re2c: {}",
        serde_json::to_string(&ts_word).unwrap(),
        serde_json::to_string(&re2c_word).unwrap(),
    );
}

#[test]
fn word_equivalence_lang_suffix() {
    let ts_word = ts().parse_word("hao3@s:zho").unwrap();
    let re2c_word = re2c_word("hao3@s:zho");
    assert!(
        ts_word.semantic_eq(&re2c_word),
        "lang suffix mismatch:\n  ts:   {}\n  re2c: {}",
        serde_json::to_string(&ts_word).unwrap(),
        serde_json::to_string(&re2c_word).unwrap(),
    );
}

// ═══════════════════════════════════════════════════════════════
// Main tier structure verification
// ═══════════════════════════════════════════════════════════════

#[test]
fn main_tier_retrace_structure() {
    // Verify our parser produces Retrace for "the the [/] dog ."
    let mt = talkbank_parser_re2c::parser::parse_main_tier("*CHI:\tthe the [/] dog .\n").unwrap();
    let has_retrace = mt
        .tier_body
        .contents
        .iter()
        .any(|c| matches!(c, talkbank_parser_re2c::ast::ContentItem::Retrace(_)));
    assert!(
        has_retrace,
        "expected Retrace in: {:?}",
        mt.tier_body.contents
    );
}

#[test]
fn main_tier_equivalence_simple() {
    let input = "*CHI:\thello world .\n";
    let ts_mt = ts().parse_main_tier(input).unwrap();
    let re2c_parsed = talkbank_parser_re2c::parser::parse_main_tier(input).unwrap();
    let re2c_mt = talkbank_parser_re2c::convert::main_tier_to_model(&re2c_parsed);
    assert!(
        ts_mt.semantic_eq(&re2c_mt),
        "simple main tier mismatch:\n  ts:   {}\n  re2c: {}",
        serde_json::to_string(&ts_mt).unwrap(),
        serde_json::to_string(&re2c_mt).unwrap(),
    );
}

#[test]
fn main_tier_equivalence_retrace() {
    let input = "*CHI:\tthe the [/] dog .\n";
    let ts_mt = ts().parse_main_tier(input).unwrap();
    let re2c_parsed = talkbank_parser_re2c::parser::parse_main_tier(input).unwrap();
    let re2c_mt = talkbank_parser_re2c::convert::main_tier_to_model(&re2c_parsed);
    assert!(
        ts_mt.semantic_eq(&re2c_mt),
        "retrace main tier mismatch:\n  ts:   {}\n  re2c: {}",
        serde_json::to_string(&ts_mt).unwrap(),
        serde_json::to_string(&re2c_mt).unwrap(),
    );
}

#[test]
fn main_tier_equivalence_compound() {
    let input = "*CHI:\tI want ice+cream .\n";
    let ts_mt = ts().parse_main_tier(input).unwrap();
    let re2c_parsed = talkbank_parser_re2c::parser::parse_main_tier(input).unwrap();
    let re2c_mt = talkbank_parser_re2c::convert::main_tier_to_model(&re2c_parsed);
    assert!(
        ts_mt.semantic_eq(&re2c_mt),
        "compound main tier mismatch:\n  ts:   {}\n  re2c: {}",
        serde_json::to_string(&ts_mt).unwrap(),
        serde_json::to_string(&re2c_mt).unwrap(),
    );
}

#[test]
fn main_tier_equivalence_event() {
    let input = "*CHI:\t&=laughs .\n";
    let ts_mt = ts().parse_main_tier(input).unwrap();
    let re2c_parsed = talkbank_parser_re2c::parser::parse_main_tier(input).unwrap();
    let re2c_mt = talkbank_parser_re2c::convert::main_tier_to_model(&re2c_parsed);
    assert!(
        ts_mt.semantic_eq(&re2c_mt),
        "event main tier mismatch:\n  ts:   {}\n  re2c: {}",
        serde_json::to_string(&ts_mt).unwrap(),
        serde_json::to_string(&re2c_mt).unwrap(),
    );
}

#[test]
fn main_tier_equivalence_pause() {
    let input = "*CHI:\tI (.) want cookies .\n";
    let ts_mt = ts().parse_main_tier(input).unwrap();
    let re2c_parsed = talkbank_parser_re2c::parser::parse_main_tier(input).unwrap();
    let re2c_mt = talkbank_parser_re2c::convert::main_tier_to_model(&re2c_parsed);
    assert!(
        ts_mt.semantic_eq(&re2c_mt),
        "pause main tier mismatch:\n  ts:   {}\n  re2c: {}",
        serde_json::to_string(&ts_mt).unwrap(),
        serde_json::to_string(&re2c_mt).unwrap(),
    );
}

#[test]
fn main_tier_equivalence_trailing_off() {
    let input = "*CHI:\tI was going to the +...\n";
    let ts_mt = ts().parse_main_tier(input).unwrap();
    let re2c_parsed = talkbank_parser_re2c::parser::parse_main_tier(input).unwrap();
    let re2c_mt = talkbank_parser_re2c::convert::main_tier_to_model(&re2c_parsed);
    assert!(
        ts_mt.semantic_eq(&re2c_mt),
        "trailing off mismatch:\n  ts:   {}\n  re2c: {}",
        serde_json::to_string(&ts_mt).unwrap(),
        serde_json::to_string(&re2c_mt).unwrap(),
    );
}

#[test]
fn mor_tier_equivalence() {
    let input = "pro|I v|want n|cookie-PL .\n";
    let errors = talkbank_model::errors::ErrorCollector::new();
    let ts_result = ts().parse_mor_tier_fragment(input, 0, &errors);
    let re2c_parsed = talkbank_parser_re2c::parser::parse_mor_tier(input);
    let re2c_tier = match talkbank_model::model::MorTier::try_from(&re2c_parsed) {
        Ok(t) => t,
        Err(e) => panic!("re2c MorTier conversion failed: {:?}", e),
    };
    if let talkbank_model::ParseOutcome::Parsed(ts_tier) = ts_result {
        assert!(
            ts_tier.semantic_eq(&re2c_tier),
            "mor tier mismatch:\n  ts:   {}\n  re2c: {}",
            serde_json::to_string(&ts_tier).unwrap(),
            serde_json::to_string(&re2c_tier).unwrap(),
        );
    } else {
        panic!("ts rejected mor tier");
    }
}

#[test]
fn gra_tier_equivalence() {
    let input = "1|2|SUBJ 2|0|ROOT 3|2|OBJ\n";
    let errors = talkbank_model::errors::ErrorCollector::new();
    let ts_result = ts().parse_gra_tier_fragment(input, 0, &errors);
    let re2c_parsed = talkbank_parser_re2c::parser::parse_gra_tier(input);
    let re2c_tier = talkbank_model::model::GraTier::from(&re2c_parsed);
    if let talkbank_model::ParseOutcome::Parsed(ts_tier) = ts_result {
        assert!(
            ts_tier.semantic_eq(&re2c_tier),
            "gra tier mismatch:\n  ts:   {}\n  re2c: {}",
            serde_json::to_string(&ts_tier).unwrap(),
            serde_json::to_string(&re2c_tier).unwrap(),
        );
    } else {
        panic!("ts rejected gra tier");
    }
}

// ═══════════════════════════════════════════════════════════════
// Full file equivalence
// ═══════════════════════════════════════════════════════════════

#[test]
fn file_equivalence_basic_conversation() {
    let path = format!(
        "{}/resources/corpus/reference/core/basic-conversation.cha",
        fixture_utils::workspace_root().display()
    );
    let content = std::fs::read_to_string(&path).expect("read basic-conversation.cha");

    let ts_file = ts().parse_chat_file(&content).expect("ts parse");
    let re2c_parsed = talkbank_parser_re2c::parser::parse_chat_file(&content);
    let re2c_file = talkbank_model::model::ChatFile::from(&re2c_parsed);

    if !ts_file.semantic_eq(&re2c_file) {
        // Show per-line comparison
        eprintln!("FILE MISMATCH: basic-conversation.cha");
        eprintln!("  ts lines: {}", ts_file.lines.len());
        eprintln!("  re2c lines: {}", re2c_file.lines.len());

        let ts_json = serde_json::to_string_pretty(&ts_file).unwrap();
        let re2c_json = serde_json::to_string_pretty(&re2c_file).unwrap();

        // Write to temp files for diff
        std::fs::write("/tmp/ts_output.json", &ts_json).ok();
        std::fs::write("/tmp/re2c_output.json", &re2c_json).ok();
        eprintln!("  diff /tmp/ts_output.json /tmp/re2c_output.json");

        panic!("semantic mismatch — see /tmp/*.json for details");
    }
}

#[test]
fn file_equivalence_reference_corpus_core() {
    let base = format!(
        "{}/resources/corpus/reference/core",
        fixture_utils::workspace_root().display()
    );
    let base_path = std::path::Path::new(&base);
    if !base_path.exists() {
        return;
    }

    let mut passed = 0;
    let mut failed = Vec::new();

    for entry in std::fs::read_dir(base_path).unwrap() {
        let path = entry.unwrap().path();
        if !path.extension().is_some_and(|e| e == "cha") {
            continue;
        }
        let filename = path.file_name().unwrap().to_string_lossy().to_string();
        let content = std::fs::read_to_string(&path).unwrap();

        let ts_file = ts().parse_chat_file(&content).unwrap();
        let re2c_parsed = talkbank_parser_re2c::parser::parse_chat_file(&content);
        let re2c_file = talkbank_model::model::ChatFile::from(&re2c_parsed);

        if ts_file.semantic_eq(&re2c_file) {
            passed += 1;
        } else {
            if failed.is_empty() {
                // Write first failure for debugging
                std::fs::write(
                    "/tmp/ts_output.json",
                    serde_json::to_string_pretty(&ts_file).unwrap(),
                )
                .ok();
                std::fs::write(
                    "/tmp/re2c_output.json",
                    serde_json::to_string_pretty(&re2c_file).unwrap(),
                )
                .ok();
            }
            failed.push(filename);
        }
    }

    eprintln!(
        "core/ equivalence: {passed} passed, {} failed",
        failed.len()
    );
    for f in &failed {
        eprintln!("  FAIL: {f}");
    }
    assert!(
        failed.is_empty(),
        "{} files failed equivalence",
        failed.len()
    );
}

#[test]
fn file_equivalence_reference_corpus_content() {
    let base = format!(
        "{}/resources/corpus/reference/content",
        fixture_utils::workspace_root().display()
    );
    let base_path = std::path::Path::new(&base);
    if !base_path.exists() {
        return;
    }

    let mut passed = 0;
    let mut failed = Vec::new();

    for entry in std::fs::read_dir(base_path).unwrap() {
        let path = entry.unwrap().path();
        if !path.extension().is_some_and(|e| e == "cha") {
            continue;
        }
        let filename = path.file_name().unwrap().to_string_lossy().to_string();
        let content = std::fs::read_to_string(&path).unwrap();

        let ts_file = ts().parse_chat_file(&content).unwrap();
        let re2c_parsed = talkbank_parser_re2c::parser::parse_chat_file(&content);
        let re2c_file = talkbank_model::model::ChatFile::from(&re2c_parsed);

        if ts_file.semantic_eq(&re2c_file) {
            passed += 1;
        } else {
            if failed.is_empty() {
                std::fs::write(
                    "/tmp/ts_output.json",
                    serde_json::to_string_pretty(&ts_file).unwrap(),
                )
                .ok();
                std::fs::write(
                    "/tmp/re2c_output.json",
                    serde_json::to_string_pretty(&re2c_file).unwrap(),
                )
                .ok();
            }
            failed.push(filename);
        }
    }

    eprintln!(
        "content/ equivalence: {passed} passed, {} failed",
        failed.len()
    );
    for f in &failed {
        eprintln!("  FAIL: {f}");
    }
    assert!(
        failed.is_empty(),
        "{} files failed equivalence",
        failed.len()
    );
}

#[test]
fn file_equivalence_reference_corpus_annotation() {
    let base = format!(
        "{}/resources/corpus/reference/annotation",
        fixture_utils::workspace_root().display()
    );
    let base_path = std::path::Path::new(&base);
    if !base_path.exists() {
        return;
    }

    let mut passed = 0;
    let mut failed = Vec::new();

    for entry in std::fs::read_dir(base_path).unwrap() {
        let path = entry.unwrap().path();
        if !path.extension().is_some_and(|e| e == "cha") {
            continue;
        }
        let filename = path.file_name().unwrap().to_string_lossy().to_string();
        let content = std::fs::read_to_string(&path).unwrap();

        let ts_file = ts().parse_chat_file(&content).unwrap();
        let re2c_parsed = talkbank_parser_re2c::parser::parse_chat_file(&content);
        let re2c_file = talkbank_model::model::ChatFile::from(&re2c_parsed);

        if ts_file.semantic_eq(&re2c_file) {
            passed += 1;
        } else {
            if failed.is_empty() {
                std::fs::write(
                    "/tmp/ts_output.json",
                    serde_json::to_string_pretty(&ts_file).unwrap(),
                )
                .ok();
                std::fs::write(
                    "/tmp/re2c_output.json",
                    serde_json::to_string_pretty(&re2c_file).unwrap(),
                )
                .ok();
            }
            failed.push(filename);
        }
    }

    eprintln!(
        "annotation/ equivalence: {passed} passed, {} failed",
        failed.len()
    );
    for f in &failed {
        eprintln!("  FAIL: {f}");
    }
    assert!(
        failed.is_empty(),
        "{} files failed equivalence",
        failed.len()
    );
}

macro_rules! corpus_equivalence_test {
    ($name:ident, $dir:expr) => {
        #[test]
        fn $name() {
            let base = format!(
                "{}/resources/corpus/reference/{}",
                fixture_utils::workspace_root().display(),
                $dir
            );
            let base_path = std::path::Path::new(&base);
            if !base_path.exists() {
                return;
            }
            let mut passed = 0;
            let mut failed = Vec::new();
            for entry in std::fs::read_dir(base_path).unwrap() {
                let path = entry.unwrap().path();
                if !path.extension().is_some_and(|e| e == "cha") {
                    continue;
                }
                let filename = path.file_name().unwrap().to_string_lossy().to_string();
                let content = std::fs::read_to_string(&path).unwrap();
                let ts_file = ts().parse_chat_file(&content).unwrap();
                let re2c_parsed = talkbank_parser_re2c::parser::parse_chat_file(&content);
                let re2c_file = talkbank_model::model::ChatFile::from(&re2c_parsed);
                if ts_file.semantic_eq(&re2c_file) {
                    passed += 1;
                } else {
                    if failed.is_empty() {
                        std::fs::write(
                            "/tmp/ts_output.json",
                            serde_json::to_string_pretty(&ts_file).unwrap(),
                        )
                        .ok();
                        std::fs::write(
                            "/tmp/re2c_output.json",
                            serde_json::to_string_pretty(&re2c_file).unwrap(),
                        )
                        .ok();
                    }
                    failed.push(filename);
                }
            }
            eprintln!(
                "{} equivalence: {passed} passed, {} failed",
                $dir,
                failed.len()
            );
            for f in &failed {
                eprintln!("  FAIL: {f}");
            }
            assert!(
                failed.is_empty(),
                "{} files failed equivalence",
                failed.len()
            );
        }
    };
}

corpus_equivalence_test!(file_equivalence_reference_corpus_tiers, "tiers");
corpus_equivalence_test!(file_equivalence_reference_corpus_ca, "ca");
corpus_equivalence_test!(file_equivalence_reference_corpus_languages, "languages");
corpus_equivalence_test!(
    file_equivalence_reference_corpus_word_features,
    "word-features"
);

#[test]
#[ignore]
fn study_retrace_cha_dep_tiers() {
    let base = format!(
        "{}/resources/corpus/reference/annotation",
        fixture_utils::workspace_root().display()
    );
    let content = std::fs::read_to_string(format!("{base}/retrace.cha")).unwrap();
    let re2c_parsed = talkbank_parser_re2c::parser::parse_chat_file(&content);
    let re2c_file = talkbank_model::model::ChatFile::from(&re2c_parsed);
    for (i, line) in re2c_file.lines.iter().enumerate() {
        if let talkbank_model::model::Line::Utterance(u) = line {
            let dep_types: Vec<_> = u
                .dependent_tiers
                .iter()
                .map(|d| format!("{:?}", std::mem::discriminant(d)))
                .collect();
            eprintln!("Line {i}: {} deps: {dep_types:?}", u.dependent_tiers.len());
        }
    }
}

/// Parse a word using our re2c parser and convert to model Word.
fn re2c_word(input: &str) -> talkbank_model::model::Word {
    let parsed = talkbank_parser_re2c::parser::parse_word(input).expect("re2c parse_word");
    talkbank_parser_re2c::convert::word_from_parsed(&parsed)
}

#[test]
#[ignore]
fn study_word_compound() {
    let w = ts().parse_word("ice+cream").unwrap();
    eprintln!("{}", serde_json::to_string_pretty(&w).unwrap());
}

#[test]
#[ignore]
fn study_word_form_marker() {
    let w = ts().parse_word("mama@f").unwrap();
    eprintln!("{}", serde_json::to_string_pretty(&w).unwrap());
}

#[test]
#[ignore]
fn study_word_lengthening() {
    let w = ts().parse_word("no::").unwrap();
    eprintln!("{}", serde_json::to_string_pretty(&w).unwrap());
}

#[test]
#[ignore]
fn study_mor_tier() {
    let errors = talkbank_model::errors::ErrorCollector::new();
    let tier = ts().parse_mor_tier_fragment("pro|I v|want n|cookie-PL .\n", 0, &errors);
    if let talkbank_model::ParseOutcome::Parsed(t) = tier {
        eprintln!("{}", serde_json::to_string_pretty(&t).unwrap());
    } else {
        eprintln!("REJECTED");
    }
}

#[test]
#[ignore]
fn study_gra_tier() {
    let errors = talkbank_model::errors::ErrorCollector::new();
    let tier = ts().parse_gra_tier_fragment("1|2|SUBJ 2|0|ROOT 3|2|OBJ .\n", 0, &errors);
    if let talkbank_model::ParseOutcome::Parsed(t) = tier {
        eprintln!("{}", serde_json::to_string_pretty(&t).unwrap());
    } else {
        eprintln!("REJECTED");
    }
}

// ═══════════════════════════════════════════════════════════════
// Single-file divergence debugger
// ═══════════════════════════════════════════════════════════════

/// Debug a single file: parse with both parsers, find first divergent utterance.
///
/// Set FILE env var or edit the default path. Run:
/// ```bash
/// FILE=path/to.cha cargo test -p talkbank-parser-re2c --test model_study -- debug_single_file --nocapture --ignored
/// ```
#[test]
#[ignore]
fn debug_single_file() {
    let path = std::env::var("FILE").unwrap_or_else(|_| {
        format!(
            "{}/talkbank/data/ca-data/Bergmann/003_Moramin1.cha",
            std::env::var("HOME").unwrap()
        )
    });
    let content = std::fs::read_to_string(&path).expect("read file");

    let ts_parser = ts();
    let re2c_parser = talkbank_parser_re2c::Re2cParser::new();
    let errors = talkbank_model::errors::ErrorCollector::new();

    let ts_file = ts_parser.parse_chat_file_streaming(&content, &errors);
    let re2c_file = match re2c_parser.parse_chat_file(&content, 0, &errors) {
        talkbank_model::ParseOutcome::Parsed(f) => f,
        _ => {
            eprintln!("Re2c REJECTED");
            return;
        }
    };

    if ts_file.semantic_eq(&re2c_file) {
        eprintln!("EQUIVALENT — no divergence");
        return;
    }

    let ts_utts: Vec<_> = ts_file.utterances().collect();
    let re2c_utts: Vec<_> = re2c_file.utterances().collect();
    eprintln!(
        "DIVERGENT: ts={} utts, re2c={} utts",
        ts_utts.len(),
        re2c_utts.len()
    );

    if ts_utts.len() != re2c_utts.len() {
        eprintln!("  Utterance COUNT mismatch — different line parsing");
    }

    for (i, (t, r)) in ts_utts.iter().zip(re2c_utts.iter()).enumerate() {
        if !t.semantic_eq(r) {
            let tc = &t.main.content;
            let rc = &r.main.content;
            eprintln!("  First divergent utterance: {i}");
            eprintln!("  Speaker: {}", t.main.speaker);
            eprintln!(
                "  TS:   {} items, bullet={}, term={:?}",
                tc.content.len(),
                tc.bullet.is_some(),
                tc.terminator.as_ref().map(|t| format!("{t:?}"))
            );
            eprintln!(
                "  Re2c: {} items, bullet={}, term={:?}",
                rc.content.len(),
                rc.bullet.is_some(),
                rc.terminator.as_ref().map(|t| format!("{t:?}"))
            );

            let max = tc.content.len().max(rc.content.len());
            for j in 0..max {
                let ts_t = tc
                    .content
                    .get(j)
                    .map(|c| format!("{:?}", std::mem::discriminant(c)))
                    .unwrap_or_else(|| "<missing>".into());
                let re_t = rc
                    .content
                    .get(j)
                    .map(|c| format!("{:?}", std::mem::discriminant(c)))
                    .unwrap_or_else(|| "<missing>".into());
                let marker = if ts_t != re_t { " <<<" } else { "" };
                eprintln!("    [{j}] ts={ts_t:30} re2c={re_t}{marker}");
            }

            if t.dependent_tiers.len() != r.dependent_tiers.len() {
                eprintln!(
                    "  Deps: ts={}, re2c={}",
                    t.dependent_tiers.len(),
                    r.dependent_tiers.len()
                );
            }
            break;
        }
    }
}

/// Diff a single file's parse between TreeSitter and Re2c. Standard
/// drill-in tool when `categorize_divergences` flags a file: writes both
/// parsers' JSON to `/tmp/re2c_compare_{ts,re2c}.json` and prints
/// `semantic_eq:`. Used during the P12 parity-push workflow.
///
///     TB_DIFF_FILE=/path/to/file.cha cargo test --offline \
///         -p talkbank-parser-re2c --test model_study --release \
///         study_diff_one_file -- --ignored --nocapture
#[test]
#[ignore]
fn study_diff_one_file() {
    use talkbank_model::ErrorCollector;
    use talkbank_model::ParseOutcome;
    use talkbank_parser_re2c::Re2cParser;

    let path = std::env::var("TB_DIFF_FILE").expect("set TB_DIFF_FILE=path");
    let content = std::fs::read_to_string(&path).unwrap();
    let tsp = ts();
    let re2c = Re2cParser::new();
    let errs = ErrorCollector::new();
    // Streaming variant returns the recovered ChatFile even when TS emits
    // diagnostics — matching what categorize_divergences and the
    // equivalence_reference_corpus oracle use.
    let ts_errors = ErrorCollector::new();
    let ts_file = tsp.parse_chat_file_streaming(&content, &ts_errors);
    let re2c_out = re2c.parse_chat_file(&content, 0, &errs);
    let re2c_file = match re2c_out {
        ParseOutcome::Parsed(f) => f,
        _ => panic!("re2c rejected"),
    };
    let eq = ts_file.semantic_eq(&re2c_file);
    eprintln!("semantic_eq: {eq}");
    let ts_json = serde_json::to_string_pretty(&ts_file).unwrap();
    let re2c_json = serde_json::to_string_pretty(&re2c_file).unwrap();
    std::fs::write("/tmp/re2c_compare_ts.json", &ts_json).unwrap();
    std::fs::write("/tmp/re2c_compare_re2c.json", &re2c_json).unwrap();
    eprintln!(
        "wrote /tmp/re2c_compare_ts.json + /tmp/re2c_compare_re2c.json (run: diff /tmp/re2c_compare_ts.json /tmp/re2c_compare_re2c.json)"
    );
}
