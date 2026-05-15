//! Tests for the forced-alignment module, partitioned by feature so
//! each child file fits the workspace's ≤800 LOC hard limit.
//!
//! Shared helpers (`parse_chat`, the Utterance accessors, fixtures
//! like `wor_timed_chat` / `proof_chat`, `make_fa_words`, etc.) live
//! here as `pub(super) fn` so each child file imports via
//! `use super::*;` without redundant copies.

#![allow(unused_imports, dead_code, ambiguous_glob_reexports)]

// Re-export `fa::*` into `fa::tests::*` so child files can `use super::*;`
// to pull in both fa internals (the implementation under test) and the
// shared helpers defined below. Without this re-export, child modules
// would have to disambiguate against sibling fa modules
// (`fa/postprocess.rs`, `fa/grouping.rs`, etc.) whose names match our
// local children.
pub(super) use super::*;

use talkbank_model::UtteranceIdx;
use talkbank_model::model::{Line, UtteranceContent, WriteChat};
use talkbank_parser::TreeSitterParser;

mod bullet_rerun;
mod find_reusable;
mod grouping_and_wor;
mod inject_and_parse;
mod postprocess_continuous;
mod replaced_word_and_compound;
mod two_pass_and_strategy;
mod update_bullet;
mod utr_and_monotonicity;

pub(super) fn parse_chat(text: &str) -> talkbank_model::model::ChatFile {
    let parser = TreeSitterParser::new().unwrap();
    parser.parse_chat_file(text).unwrap()
}

pub(super) fn get_test_utterance(
    chat: &mut talkbank_model::model::ChatFile,
    idx: usize,
) -> &mut talkbank_model::model::Utterance {
    let mut utt_idx = 0;
    for line in &mut chat.lines {
        if let Line::Utterance(utt) = line {
            if utt_idx == idx {
                return utt;
            }
            utt_idx += 1;
        }
    }
    panic!("Utterance {idx} not found");
}

pub(super) fn get_utterance(
    chat: &talkbank_model::model::ChatFile,
    idx: usize,
) -> &talkbank_model::model::Utterance {
    let mut utt_idx = 0;
    for line in &chat.lines {
        if let Line::Utterance(utt) = line {
            if utt_idx == idx {
                return utt;
            }
            utt_idx += 1;
        }
    }
    panic!("Utterance {idx} not found");
}

pub(super) fn wor_timed_chat() -> String {
    "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI|||||Target_Child|||\n*CHI:\thello world .\n%wor:\thello \u{15}100_500\u{15} world \u{15}600_1000\u{15} .\n@End\n".to_string()
}

pub(super) fn proof_chat(main: &str) -> String {
    format!(
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI|||||Target_Child|||\n*CHI:\t{main}\n@End\n"
    )
}

pub(super) fn collect_proof_fa_words(main: &str) -> Vec<String> {
    let chat = parse_chat(&proof_chat(main));
    let utt = get_utterance(&chat, 0);
    let mut out = Vec::new();
    collect_fa_words(&utt.main.content.content, &mut out);
    out
}

pub(super) fn generate_proof_wor_words(main: &str) -> Vec<String> {
    let chat = parse_chat(&proof_chat(main));
    let utt = get_utterance(&chat, 0);
    utt.main
        .generate_wor_tier()
        .words()
        .map(|word| word.cleaned_text().to_string())
        .collect()
}

pub(super) fn words(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_string()).collect()
}

pub(super) fn make_fa_words(texts: &[&str]) -> Vec<FaWord> {
    texts
        .iter()
        .enumerate()
        .map(|(i, t)| FaWord {
            utterance_index: UtteranceIdx(0),
            utterance_word_index: WordIdx(i),
            text: t.to_string(),
        })
        .collect()
}

pub(super) fn make_utr_tokens(words_with_times: &[(&str, u64, u64)]) -> Vec<utr::AsrTimingToken> {
    words_with_times
        .iter()
        .map(|(text, start, end)| utr::AsrTimingToken {
            text: text.to_string(),
            start_ms: *start,
            end_ms: *end,
        })
        .collect()
}

pub(super) fn get_utterance_bullet(
    chat: &talkbank_model::model::ChatFile,
    idx: usize,
) -> Option<(u64, u64)> {
    let mut utt_idx = 0;
    for line in &chat.lines {
        if let Line::Utterance(utt) = line {
            if utt_idx == idx {
                return utt
                    .main
                    .content
                    .bullet
                    .as_ref()
                    .map(|b| (b.timing.start_ms, b.timing.end_ms));
            }
            utt_idx += 1;
        }
    }
    None
}

pub(super) fn count_internal_bullets(chat: &talkbank_model::model::ChatFile) -> usize {
    let mut count = 0;
    for line in &chat.lines {
        if let Line::Utterance(utt) = line {
            for item in &utt.main.content.content.0 {
                if matches!(item, UtteranceContent::InternalBullet(_)) {
                    count += 1;
                }
            }
        }
    }
    count
}

pub(super) fn count_double_bullet_lines(chat_text: &str) -> usize {
    chat_text
        .lines()
        .filter(|line| line.starts_with('*'))
        .filter(|line| {
            let bullet_count = line.matches('\x15').count() / 2; // each bullet is a pair
            bullet_count > 1
        })
        .count()
}
