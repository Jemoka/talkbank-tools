//! Test module for utterance in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use talkbank_model::Span;
use talkbank_model::model::{
    BulletContent, ChatFile, ComTier, GraTier, GrammaticalRelation, Header, LanguageCode, Line,
    MainTier, Terminator, Utterance, UtteranceContent, Word,
};

/// Verifies a minimal utterance serializes to expected CHAT main-tier text.
#[test]
fn utterance_round_trip_simple() {
    let main = MainTier::new(
        "CHI",
        vec![UtteranceContent::Word(Box::new(Word::new_unchecked(
            "hello", "hello",
        )))],
        Terminator::Period { span: Span::DUMMY },
    );
    let utterance = Utterance::new(main);
    let output = utterance.to_chat();
    assert!(
        output.contains("*CHI:\thello ."),
        "Expected main tier in output: {}",
        output
    );
}

/// Verifies utterances with attached dependent tiers serialize all tiers correctly.
#[test]
fn utterance_round_trip_with_dependent_tiers() {
    let main = MainTier::new(
        "CHI",
        vec![UtteranceContent::Word(Box::new(Word::new_unchecked(
            "hello", "hello",
        )))],
        Terminator::Period { span: Span::DUMMY },
    );

    let utterance = Utterance::new(main)
        .with_gra(GraTier::new_gra(vec![GrammaticalRelation::new(
            1, 0, "ROOT",
        )]))
        .with_com(ComTier::new(BulletContent::from_text("comment text")));

    let output = utterance.to_chat();
    assert!(output.contains("*CHI:\thello ."), "Expected main tier");
    assert!(output.contains("%gra:\t1|0|ROOT"), "Expected gra tier");
    assert!(output.contains("%com:\tcomment text"), "Expected com tier");
}

/// Verifies a minimal `ChatFile` with headers and one utterance round-trips.
#[test]
fn chat_file_round_trip() {
    let lines = vec![
        Line::header(Header::Utf8),
        Line::header(Header::Languages {
            codes: vec![LanguageCode::new("eng")].into(),
        }),
        Line::utterance(Utterance::new(MainTier::new(
            "CHI",
            vec![UtteranceContent::Word(Box::new(Word::new_unchecked(
                "hello", "hello",
            )))],
            Terminator::Period { span: Span::DUMMY },
        ))),
    ];

    let file = ChatFile::new(lines);
    let output = file.to_chat();

    assert!(output.contains("@UTF8"), "Expected UTF8 header");
    assert!(
        output.contains("@Languages:\teng"),
        "Expected Languages header"
    );
    assert!(output.contains("*CHI:\thello ."), "Expected utterance");
}
