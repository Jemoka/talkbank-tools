//! Test module for test debug error group in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use talkbank_model::model::{Line, UtteranceContent};
use talkbank_parser::TreeSitterParser;

/// Verifies `<...> [*]` error groups parse as annotated groups with inner words.
#[test]
fn error_marker_group_words_are_parsed() {
    let content = "@UTF8\n@Begin\n@Languages:\thrv\n@Participants:\tPAR Participant\n@ID:\thrv|test|PAR|||||Participant|||\n*PAR:\thello <one two three> [*] .\n%mor:\tn|hello n|one n|two n|three .\n@End\n";

    let parser = TreeSitterParser::new().expect("grammar loads");
    let chat_file = parser
        .parse_chat_file(content)
        .expect("parse should succeed");

    for line in &chat_file.lines {
        if let Line::Utterance(u) = line {
            // Parser should produce 2 content items: Word("hello") + AnnotatedGroup(<one two three> [*])
            assert_eq!(
                u.main.content.content.len(),
                2,
                "Expected 2 content items (word + annotated group)"
            );

            // The annotated group should be an AnnotatedGroup with Error annotation and 3 inner words
            match &u.main.content.content[1] {
                UtteranceContent::AnnotatedGroup(a) => {
                    assert_eq!(
                        a.inner.content.content.len(),
                        3,
                        "Group should have 3 inner words"
                    );
                }
                other => panic!(
                    "Expected AnnotatedGroup, got {:?}",
                    std::mem::discriminant(other)
                ),
            }
        }
    }
}
