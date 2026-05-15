//! Parity test: `Utterance::mor_alignable_word_count()` must equal
//! `extract::collect_utterance_content(..., TierDomain::Mor, …).len()`
//! for every utterance in every reference-corpus file.
//!
//! Two independent walkers define "Mor-alignable" today:
//!
//! 1. `talkbank_model::model::Utterance::mor_alignable_word_count()` — a
//!    method that delegates to
//!    `talkbank_model::alignment::helpers::count_tier_positions`, which
//!    has its own recursive walker over `UtteranceContent` /
//!    `BracketedItem`.
//! 2. `batchalign::chat_ops::extract::collect_utterance_content` — a
//!    separate walker built on `walk_words`, applying
//!    `counts_for_tier` + `is_tag_marker_separator` per leaf.
//!
//! Both apply the rules in `alignment/helpers/rules.rs`, but if either
//! walker ever drifts (one forgets a variant, one mishandles retrace,
//! one drops tag-marker separators), the count-equality invariant at
//! `inject_morphosyntax` starts lying: the validator says "N ≠ M" when
//! the pipeline actually produced N Mors for N CHAT words, just by a
//! different count of N.
//!
//! This test fails loudly on any such drift, across the full reference
//! corpus of 98 files in 20 languages. Reference-corpus coverage is the
//! strongest gate we have on the two walkers agreeing; new variants or
//! new edge cases added to either side should add a reference fixture
//! that exercises them.

use std::path::PathBuf;

use talkbank_model::alignment::helpers::TierDomain;
use talkbank_model::model::{ChatFile, Line};
use talkbank_parser::TreeSitterParser;
use talkbank_transform::extract;
use walkdir::WalkDir;

fn reference_corpus_root() -> PathBuf {
    // batchalign-chat-ops/ tests/ ../ ../ talkbank-tools/ corpus/ reference
    // Cargo runs integration tests with CARGO_MANIFEST_DIR set to the
    // crate dir, so we resolve relative to that.
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("../talkbank-tools/resources/corpus/reference"))
        .expect("CARGO_MANIFEST_DIR should have at least two parents")
}

fn collect_cha_files(root: &PathBuf) -> Vec<PathBuf> {
    WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file() && e.path().extension().and_then(|x| x.to_str()) == Some("cha")
        })
        .map(|e| e.into_path())
        .collect()
}

fn parse_file(parser: &TreeSitterParser, path: &PathBuf) -> Option<ChatFile> {
    let contents = std::fs::read_to_string(path).ok()?;
    parser.parse_chat_file(&contents).ok()
}

/// One row per mismatch: enough information to investigate without
/// re-running the test.
#[derive(Debug)]
struct Mismatch {
    path: PathBuf,
    utt_idx: usize,
    speaker: String,
    model_count: usize,
    extract_count: usize,
    extract_words: Vec<String>,
}

#[test]
fn mor_alignable_count_parity_across_reference_corpus() {
    let corpus_root = reference_corpus_root();
    assert!(
        corpus_root.exists(),
        "reference corpus missing at {}; clones may need `make clone`",
        corpus_root.display(),
    );

    let files = collect_cha_files(&corpus_root);
    assert!(
        !files.is_empty(),
        "reference corpus at {} yielded zero .cha files",
        corpus_root.display(),
    );

    let parser = TreeSitterParser::new().expect("tree-sitter parser");

    let mut mismatches: Vec<Mismatch> = Vec::new();
    let mut total_utterances: usize = 0;
    let mut files_parsed: usize = 0;

    for path in &files {
        let Some(chat) = parse_file(&parser, path) else {
            // A few reference-corpus files are intentionally malformed
            // (parser-error fixtures). Skip them rather than fail —
            // this test is about alignment parity on parseable content.
            continue;
        };
        files_parsed += 1;

        let mut utt_idx = 0usize;
        for line in &chat.lines {
            let Line::Utterance(utt) = line else { continue };
            total_utterances += 1;

            let model_count = utt.mor_alignable_word_count();

            let mut extracted = Vec::new();
            extract::collect_utterance_content(
                &utt.main.content.content,
                TierDomain::Mor,
                &mut extracted,
            );
            let extract_count = extracted.len();

            if model_count.get() != extract_count {
                mismatches.push(Mismatch {
                    path: path.clone(),
                    utt_idx,
                    speaker: utt.main.speaker.as_str().to_string(),
                    model_count: model_count.get(),
                    extract_count,
                    extract_words: extracted
                        .iter()
                        .map(|w| w.text.as_ref().to_string())
                        .collect(),
                });
            }

            utt_idx += 1;
        }
    }

    assert!(
        files_parsed > 0,
        "zero reference files parsed cleanly; something is wrong with the parser \
         or the corpus layout",
    );

    if !mismatches.is_empty() {
        // Print the first N mismatches so developers can see the failure
        // pattern without spelunking logs.
        let show_limit = 10;
        let shown = mismatches.len().min(show_limit);
        let mut msg = format!(
            "Mor-alignable count mismatch between Utterance::mor_alignable_word_count() \
             (talkbank-model) and extract::collect_utterance_content(.., Mor, ..) \
             (batchalign-chat-ops) on {} of {total_utterances} utterances across {files_parsed} files. \
             First {shown}:\n",
            mismatches.len()
        );
        for m in mismatches.iter().take(show_limit) {
            msg.push_str(&format!(
                "  {} utt#{} *{}*: model={} extract={} extract_words={:?}\n",
                m.path.display(),
                m.utt_idx,
                m.speaker,
                m.model_count,
                m.extract_count,
                m.extract_words,
            ));
        }
        panic!("{msg}");
    }
}
