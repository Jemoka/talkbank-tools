//! CHAT serialization helper for transform-oriented callers.

use talkbank_model::WriteChat;
use talkbank_model::model::ChatFile;

/// Serialize a `ChatFile` back to CHAT text.
pub fn to_chat_string(chat_file: &ChatFile) -> String {
    chat_file.to_chat_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{TreeSitterParser, parse_lenient};

    const MINIMAL_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
@ID:\teng|test|CHI||female|||Target_Child|||\n*CHI:\thello .\n@End\n";

    const MOR_GRA_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
@ID:\teng|test|CHI|3;|male|||Target_Child|||\n*CHI:\thello world .\n\
%mor:\tn|hello n|world .\n%gra:\t1|2|SUBJ 2|0|ROOT 3|2|PUNCT\n@End\n";

    fn parse_and_serialize(chat_text: &str) -> String {
        let parser = TreeSitterParser::new().expect("tree-sitter parser");
        let (chat_file, _) = parse_lenient(&parser, chat_text);
        to_chat_string(&chat_file)
    }

    #[test]
    fn minimal_chat_round_trips() {
        let output = parse_and_serialize(MINIMAL_CHAT);
        assert_eq!(
            output, MINIMAL_CHAT,
            "minimal CHAT should round-trip exactly"
        );
    }

    #[test]
    fn chat_with_dependent_tiers_round_trips() {
        let output = parse_and_serialize(MOR_GRA_CHAT);
        assert_eq!(
            output, MOR_GRA_CHAT,
            "CHAT with %mor/%gra should round-trip"
        );
    }
}
