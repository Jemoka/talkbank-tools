//! Code lens — utterance counts per speaker above `@Participants`.
//!
//! Displays a lens like "CHI: 42 utterances" above the `@Participants` header,
//! giving annotators an at-a-glance summary of transcript composition.

use talkbank_model::model::ChatFile;
use tower_lsp::lsp_types::*;

/// Generates code lenses showing utterance count per speaker above @Participants.
pub fn code_lens(chat_file: &ChatFile, doc: &str) -> Option<Vec<CodeLens>> {
    // Count utterances per speaker.
    let mut counts: Vec<(String, usize)> = Vec::new();
    for (code, _participant) in &chat_file.participants {
        let speaker_str = code.as_str();
        let count = chat_file
            .utterances()
            .filter(|u| u.main.speaker.as_str() == speaker_str)
            .count();
        counts.push((speaker_str.to_string(), count));
    }

    if counts.is_empty() {
        return None;
    }

    // Find the @Participants header line.
    let participants_line = find_participants_line(doc)?;
    let position = Position {
        line: participants_line,
        character: 0,
    };
    let range = Range {
        start: position,
        end: position,
    };

    // Build one lens per speaker with the count.
    let mut lenses = Vec::new();
    for (speaker, count) in &counts {
        let plural = if *count == 1 {
            "utterance"
        } else {
            "utterances"
        };
        lenses.push(CodeLens {
            range,
            command: Some(Command {
                title: format!("{speaker}: {count} {plural}"),
                command: String::new(),
                arguments: None,
            }),
            data: None,
        });
    }

    Some(lenses)
}

/// Finds the 0-indexed line number of the `@Participants:` header.
fn find_participants_line(doc: &str) -> Option<u32> {
    for (i, line) in doc.lines().enumerate() {
        if line.starts_with("@Participants:") {
            return Some(i as u32);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::parse_chat;

    #[test]
    fn test_code_lens_shows_utterance_counts() {
        let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child, MOT Mother\n@ID:\teng|corpus|CHI|||||Child|||\n@ID:\teng|corpus|MOT|||||Mother|||\n*CHI:\thello .\n*MOT:\thi .\n*CHI:\tmore .\n@End\n";
        let chat_file = parse_chat(input);
        let lenses = code_lens(&chat_file, input);
        assert!(lenses.is_some());

        let lenses = lenses.unwrap();
        assert_eq!(lenses.len(), 2);

        // CHI has 2 utterances
        assert!(lenses.iter().any(|l| {
            l.command
                .as_ref()
                .unwrap()
                .title
                .contains("CHI: 2 utterances")
        }));
        // MOT has 1 utterance
        assert!(lenses.iter().any(|l| {
            l.command
                .as_ref()
                .unwrap()
                .title
                .contains("MOT: 1 utterance")
        }));
    }

    #[test]
    fn test_code_lens_none_without_participants() {
        let input = "@UTF8\n@Begin\n*CHI:\thello .\n@End\n";
        let chat_file = parse_chat(input);
        let lenses = code_lens(&chat_file, input);
        assert!(lenses.is_none());
    }
}
