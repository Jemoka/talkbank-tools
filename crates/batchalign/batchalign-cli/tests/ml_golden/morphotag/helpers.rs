use batchalign::chat_ops::{ChatFile, DependentTier};
use talkbank_transform::parse::{TreeSitterParser, parse_lenient};

pub(super) fn repeated_chat(lang: &str, speaker: &str, stem: &str, utterances: usize) -> String {
    let mut chat = format!(
        "@UTF8\n@Begin\n@Languages:\t{lang}\n@Participants:\t{speaker} Participant\n@ID:\t{lang}|test|{speaker}|||||Participant|||\n"
    );
    for i in 0..utterances {
        chat.push_str(&format!("*{speaker}:\t{stem} number {i} today .\n"));
    }
    chat.push_str("@End\n");
    chat
}

pub(super) fn find_mor_line_for(chat: &str, surface: &str) -> Option<String> {
    let lines: Vec<&str> = chat.lines().collect();
    for (idx, line) in lines.iter().enumerate() {
        if line.contains(surface)
            && line.starts_with('*')
            && idx + 1 < lines.len()
            && lines[idx + 1].starts_with("%mor:")
        {
            return Some(lines[idx + 1].trim_start_matches("%mor:\t").to_string());
        }
    }
    None
}

pub(super) fn count_mor_lines(chat: &str) -> usize {
    chat.lines()
        .filter(|line| line.starts_with("%mor:"))
        .count()
}

pub(super) fn minimal_chat(lang: &str, speaker: &str, utterance: &str) -> String {
    format!(
        "@UTF8\n@Begin\n@Languages:\t{lang}\n@Participants:\t{speaker} Participant\n@ID:\t{lang}|test|{speaker}|||||Participant|||\n*{speaker}:\t{utterance} .\n@End\n"
    )
}

pub(super) fn parse_output(chat: &str, label: &str) -> ChatFile {
    let parser = TreeSitterParser::new().unwrap();
    let (file, errors) = parse_lenient(&parser, chat);
    assert!(errors.is_empty(), "{label}: CHAT parse errors: {errors:?}");
    file
}

pub(super) fn has_mor_tier(file: &ChatFile) -> bool {
    file.lines.iter().any(|line| {
        if let batchalign::chat_ops::Line::Utterance(utt) = line {
            utt.dependent_tiers
                .iter()
                .any(|t| matches!(t, DependentTier::Mor(_)))
        } else {
            false
        }
    })
}

pub(super) fn count_ast_mor_tiers(file: &ChatFile) -> usize {
    file.lines
        .iter()
        .filter(|line| {
            if let batchalign::chat_ops::Line::Utterance(utt) = line {
                utt.dependent_tiers
                    .iter()
                    .any(|t| matches!(t, DependentTier::Mor(_)))
            } else {
                false
            }
        })
        .count()
}

pub(super) fn strip_ba3_comments(chat: &str) -> String {
    chat.lines()
        .filter(|line| !line.contains("[ba3 "))
        .collect::<Vec<_>>()
        .join("\n")
}
