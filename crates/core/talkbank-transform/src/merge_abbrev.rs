//! Merge consecutive single-letter words that form known abbreviations.
//!
//! ASR engines typically emit abbreviations as individual letters (for example
//! `*CHI: F B I .`). This transform collapses those letter sequences back into
//! one word when the concatenation matches a known abbreviation list.

use std::collections::HashSet;
use std::sync::LazyLock;

use talkbank_model::model::{ChatFile, Line, UtteranceContent, Word};

/// Known abbreviations loaded from the embedded JSON list.
#[allow(clippy::expect_used)]
static ABBREV: LazyLock<HashSet<String>> = LazyLock::new(|| {
    let data: Vec<String> = serde_json::from_str(include_str!("../data/abbrev.json"))
        .expect("embedded abbrev.json is valid");
    data.into_iter().map(|s| s.to_uppercase()).collect()
});

/// Merge consecutive single-letter words matching known abbreviations.
pub fn merge_abbreviations(chat_file: &mut ChatFile) {
    for line in &mut chat_file.lines {
        if let Line::Utterance(utt) = line {
            merge_in_content_items(&mut utt.main.content.content.0);
        }
    }
}

fn merge_in_content_items(items: &mut Vec<UtteranceContent>) {
    if items.len() < 2 {
        return;
    }

    let mut result: Vec<UtteranceContent> = Vec::with_capacity(items.len());
    let mut i = 0;

    while i < items.len() {
        let run_start = i;
        let mut letters: Vec<String> = Vec::new();

        while i < items.len() {
            if let Some(letter) = single_letter_word(&items[i]) {
                letters.push(letter);
                i += 1;
            } else {
                break;
            }
        }

        if letters.len() < 2 {
            if !letters.is_empty() {
                result.push(items[run_start].clone());
            } else {
                result.push(items[i].clone());
                i += 1;
            }
            continue;
        }

        let mut j = 0;
        while j < letters.len() {
            let mut matched = false;
            let max_len = letters.len() - j;
            for len in (2..=max_len).rev() {
                let candidate: String = letters[j..j + len]
                    .iter()
                    .map(|s| s.to_uppercase())
                    .collect();
                if ABBREV.contains(&candidate) {
                    let merged_text: String = letters[j..j + len].concat();
                    result.push(UtteranceContent::Word(Box::new(Word::simple(merged_text))));
                    j += len;
                    matched = true;
                    break;
                }
            }

            if !matched {
                result.push(items[run_start + j].clone());
                j += 1;
            }
        }
    }

    *items = result;
}

fn single_letter_word(item: &UtteranceContent) -> Option<String> {
    match item {
        UtteranceContent::Word(w) => {
            let text = w.cleaned_text();
            let mut chars = text.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) if c.is_alphabetic() => Some(c.to_string()),
                _ => None,
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::merge_abbreviations;
    use talkbank_model::model::{Terminator, WriteChat};
    use talkbank_parser::TreeSitterParser;

    fn merge_and_serialize(chat: &str) -> String {
        let parser = TreeSitterParser::new().unwrap();
        let mut file = parser.parse_chat_file(chat).unwrap();
        merge_abbreviations(&mut file);
        file.to_chat_string()
    }

    fn main_tier_words(chat_output: &str) -> Vec<String> {
        chat_output
            .lines()
            .filter(|l| l.starts_with('*'))
            .flat_map(|l| {
                let after_colon = l.split_once(':').map(|x| x.1).unwrap_or("");
                after_colon
                    .split_whitespace()
                    .filter(|w| !w.starts_with('\u{15}') && !Terminator::is_chat_terminator(w))
                    .map(String::from)
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    fn minimal_chat(utterance: &str) -> String {
        format!(
            "@UTF8\n\
             @Begin\n\
             @Languages:\teng\n\
             @Participants:\tCHI Target_Child\n\
             @ID:\teng|test|CHI|||||Target_Child|||\n\
             *CHI:\t{utterance}\n\
             @End\n"
        )
    }

    #[test]
    fn merge_fbi() {
        let chat = minimal_chat("the F B I is here .");
        let out = merge_and_serialize(&chat);
        let words = main_tier_words(&out);
        assert!(words.contains(&"FBI".to_string()));
        assert!(!words.contains(&"F".to_string()));
    }

    #[test]
    fn no_merge_unknown_tail() {
        let chat = minimal_chat("X Y Z Q W .");
        let out = merge_and_serialize(&chat);
        let words = main_tier_words(&out);
        assert!(words.contains(&"XYZ".to_string()));
        assert!(words.contains(&"Q".to_string()));
        assert!(words.contains(&"W".to_string()));
    }
}
