//! Regression test: single-word retraces with replacement must produce Retrace nodes.
//!
//! When a word has both a replacement `[: ...]` and a retrace marker `[//]`,
//! the parser must wrap the ReplacedWord inside a `UtteranceContent::Retrace`
//! node. Without this, the retraced word is counted for %mor alignment, causing
//! false E705 errors across thousands of corpus files.
//!
//! This test was written RED-first to catch a specific regression where the
//! replacement branch in `parse_word_content()` took priority over the retrace
//! branch, silently discarding the retrace marker.
//!
//! See: `crates/talkbank-parser/src/parser/tree_parsing/main/content/word.rs`

use talkbank_model::model::{Line, RetraceKind, UtteranceContent};
use talkbank_model::{ErrorCollector, ParseOutcome, ParseValidateOptions};
use talkbank_parser::TreeSitterParser;

/// Parse a minimal CHAT document and return the content items from the first
/// utterance spoken by the given speaker.
fn parse_first_utterance_content(chat: &str, speaker: &str) -> Vec<UtteranceContent> {
    let parser = TreeSitterParser::new().expect("parser creation");
    let errors = ErrorCollector::new();
    let file = match parser.parse_chat_file_fragment(chat, 0, &errors) {
        ParseOutcome::Parsed(f) => f,
        ParseOutcome::Rejected => panic!("parser rejected input"),
    };

    for line in &file.lines {
        if let Line::Utterance(utt) = line
            && utt.main.speaker.as_str() == speaker
        {
            return utt.main.content.content.0.clone();
        }
    }
    panic!("no utterance found for speaker {speaker}");
}

/// Count how many top-level content items are Retrace nodes.
fn count_retraces(content: &[UtteranceContent]) -> usize {
    content
        .iter()
        .filter(|item| matches!(item, UtteranceContent::Retrace(_)))
        .count()
}

/// Count how many top-level content items are ReplacedWord nodes
/// (i.e., NOT wrapped in a Retrace).
fn count_bare_replaced_words(content: &[UtteranceContent]) -> usize {
    content
        .iter()
        .filter(|item| matches!(item, UtteranceContent::ReplacedWord(_)))
        .count()
}

// ---------------------------------------------------------------------------
// RED tests: single-word retrace with replacement [: ...] + [//]
// ---------------------------------------------------------------------------

/// `word [: replacement] [* error] [//] correction` — the retrace must wrap
/// the replaced word, not be silently discarded.
#[test]
fn single_word_retrace_with_replacement_full() {
    let chat = "\
@UTF8
@Begin
@Participants:\tCHI Child
@Languages:\teng
*CHI:\ttika@u [: kitty] [* p:n] [//] kitty is nice .
@End
";
    let content = parse_first_utterance_content(chat, "CHI");

    // The first content item must be a Retrace wrapping the replaced word
    assert_eq!(
        count_retraces(&content),
        1,
        "expected 1 Retrace node, got content types: {:?}",
        content.iter().map(variant_name).collect::<Vec<_>>()
    );

    // The retrace must NOT appear as a bare ReplacedWord
    assert_eq!(
        count_bare_replaced_words(&content),
        0,
        "replaced word inside retrace must not appear as bare ReplacedWord"
    );

    // Verify the retrace kind
    if let UtteranceContent::Retrace(retrace) = &content[0] {
        assert_eq!(
            retrace.kind,
            RetraceKind::Full,
            "expected [//] = Full retrace"
        );
    }
}

/// `word [: replacement] [* error] [/] word [: replacement]` — partial retrace
/// (repetition) with replacement on both sides.
#[test]
fn single_word_retrace_with_replacement_partial() {
    let chat = "\
@UTF8
@Begin
@Participants:\tCHI Child
@Languages:\teng
*CHI:\tmale [: female] [* s:r] [/] male [: female] [* s:r] .
@End
";
    let content = parse_first_utterance_content(chat, "CHI");

    // First item: Retrace (the repeated word)
    assert_eq!(
        count_retraces(&content),
        1,
        "expected 1 Retrace node for [/], got content types: {:?}",
        content.iter().map(variant_name).collect::<Vec<_>>()
    );

    if let UtteranceContent::Retrace(retrace) = &content[0] {
        assert_eq!(
            retrace.kind,
            RetraceKind::Partial,
            "expected [/] = Partial retrace"
        );
    }

    // Second item: bare ReplacedWord (the correction, not retraced)
    assert_eq!(
        count_bare_replaced_words(&content),
        1,
        "the correction after [/] should be a bare ReplacedWord"
    );
}

/// `word [: replacement] [* error] [///] correction` — multiple retrace.
#[test]
fn single_word_retrace_with_replacement_multiple() {
    let chat = "\
@UTF8
@Begin
@Participants:\tCHI Child
@Languages:\teng
*CHI:\tpetté [: perché] [* p:n-rep] [///] chi lo sa ?
@End
";
    let content = parse_first_utterance_content(chat, "CHI");

    assert_eq!(
        count_retraces(&content),
        1,
        "expected 1 Retrace node for [///], got content types: {:?}",
        content.iter().map(variant_name).collect::<Vec<_>>()
    );

    if let UtteranceContent::Retrace(retrace) = &content[0] {
        assert_eq!(
            retrace.kind,
            RetraceKind::Multiple,
            "expected [///] = Multiple retrace"
        );
    }
}

/// `word [: replacement] [//] correction` — retrace with replacement but no
/// error marker. Still must produce a Retrace node.
#[test]
fn single_word_retrace_with_replacement_no_error_marker() {
    let chat = "\
@UTF8
@Begin
@Participants:\tMOT Mother
@Languages:\teng
*MOT:\twhatdya [: what do you] [///] how do you make it ?
@End
";
    let content = parse_first_utterance_content(chat, "MOT");

    assert_eq!(
        count_retraces(&content),
        1,
        "expected 1 Retrace node, got content types: {:?}",
        content.iter().map(variant_name).collect::<Vec<_>>()
    );
}

// ---------------------------------------------------------------------------
// Sanity: single-word retraces WITHOUT replacement still work
// ---------------------------------------------------------------------------

/// Plain `word [//] correction` (no replacement) — baseline.
#[test]
fn single_word_retrace_without_replacement() {
    let chat = "\
@UTF8
@Begin
@Participants:\tCHI Child
@Languages:\teng
*CHI:\tI [//] he wants it .
@End
";
    let content = parse_first_utterance_content(chat, "CHI");

    assert_eq!(
        count_retraces(&content),
        1,
        "expected 1 Retrace node, got content types: {:?}",
        content.iter().map(variant_name).collect::<Vec<_>>()
    );

    if let UtteranceContent::Retrace(retrace) = &content[0] {
        assert_eq!(retrace.kind, RetraceKind::Full);
    }
}

// ---------------------------------------------------------------------------
// Alignment: retraced replaced words must NOT count for %mor
// ---------------------------------------------------------------------------

/// Full pipeline: parse + validate must NOT emit E705 for a correctly-aligned
/// utterance where the retrace contains a replaced word.
#[test]
fn retrace_with_replacement_does_not_cause_e705() {
    let chat = "\
@UTF8
@Begin
@Participants:\tCHI Child
@Languages:\teng
@ID:\teng|test|CHI|||||Child|||
*CHI:\ttika@u [: kitty] [* p:n] [//] kitty is nice .
%mor:\tnoun|kitty aux|be-Fin-Ind-Pres-S3 adj|nice-S1 .
%gra:\t1|3|NSUBJ 2|3|COP 3|0|ROOT 4|3|PUNCT
@End
";
    let options = ParseValidateOptions {
        validate: true,
        alignment: true,
        strict_linkers: false,
    };

    let result = talkbank_transform::parse_and_validate(chat, options);

    match result {
        Ok(_) => {} // No errors — correct
        Err(e) => {
            let msg = format!("{e}");
            assert!(
                !msg.contains("E705"),
                "retrace with replacement should not trigger E705 alignment error, but got: {msg}"
            );
        }
    }
}

fn variant_name(item: &UtteranceContent) -> &'static str {
    match item {
        UtteranceContent::Word(_) => "Word",
        UtteranceContent::ReplacedWord(_) => "ReplacedWord",
        UtteranceContent::Retrace(_) => "Retrace",
        UtteranceContent::AnnotatedWord(_) => "AnnotatedWord",
        UtteranceContent::AnnotatedGroup(_) => "AnnotatedGroup",
        UtteranceContent::Group(_) => "Group",
        UtteranceContent::Separator(_) => "Separator",
        UtteranceContent::Pause(_) => "Pause",
        UtteranceContent::Event(_) => "Event",
        UtteranceContent::AnnotatedEvent(_) => "AnnotatedEvent",
        UtteranceContent::AnnotatedAction(_) => "AnnotatedAction",
        UtteranceContent::Freecode(_) => "Freecode",
        UtteranceContent::OverlapPoint(_) => "OverlapPoint",
        UtteranceContent::InternalBullet(_) => "InternalBullet",
        UtteranceContent::PhoGroup(_) => "PhoGroup",
        UtteranceContent::SinGroup(_) => "SinGroup",
        UtteranceContent::Quotation(_) => "Quotation",
        UtteranceContent::LongFeatureBegin(_) => "LongFeatureBegin",
        UtteranceContent::LongFeatureEnd(_) => "LongFeatureEnd",
        UtteranceContent::UnderlineBegin(_) => "UnderlineBegin",
        UtteranceContent::UnderlineEnd(_) => "UnderlineEnd",
        UtteranceContent::NonvocalBegin(_) => "NonvocalBegin",
        UtteranceContent::NonvocalEnd(_) => "NonvocalEnd",
        UtteranceContent::NonvocalSimple(_) => "NonvocalSimple",
        UtteranceContent::OtherSpokenEvent(_) => "OtherSpokenEvent",
    }
}
