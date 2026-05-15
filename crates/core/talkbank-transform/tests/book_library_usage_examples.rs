//! Compile + run the code examples shown in
//! `book/src/chatter/integrating/library-usage.md`.
//!
//! The book renders those examples as `rust,ignore` fences because
//! mdbook test can't disambiguate among the workspace's many compiled
//! `libtalkbank_transform-HASH.rlib` candidates in the shared
//! `target/debug/deps/` directory (it lacks Cargo's per-dep `--extern`
//! machinery). To keep the examples honest against API drift, the
//! same code lives here as a real integration test that Cargo runs
//! with the correct extern wiring.
//!
//! If you change library-usage.md, mirror the change here. Conversely,
//! if these tests start failing after a library API change, the book
//! is now drifted and needs the matching edit.
//!
//! Per-example helper functions are public-but-not-exported (this is
//! a tests/ binary), one per book section, so a compile error
//! diagnostic points at a single example.

use talkbank_model::{DependentTier, ErrorSink, ParseError, ParseValidateOptions, WriteChat};
use talkbank_parser::TreeSitterParser;
use talkbank_transform::json::to_json_pretty_validated;
use talkbank_transform::{PipelineError, parse_and_validate, parse_and_validate_with_parser};

const SAMPLE_MINIMAL: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI|||||Target_Child|||\n*CHI:\thello .\n@End\n";

const SAMPLE_WITH_MOR: &str = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|test|CHI|||||Target_Child|||
*CHI:\thello world .
%mor:\tco|hello n|world .
@End
";

/// Book example: "Parsing and Validating a CHAT File" (simplest entry).
#[test]
fn book_parsing_and_validating_simplest() -> Result<(), PipelineError> {
    let source = SAMPLE_MINIMAL.to_owned();
    let options = ParseValidateOptions::default().with_validation();
    let chat_file = parse_and_validate(&source, options)?;

    // The book uses println!; the assertion checks the example's claim
    // that utterances are iterable and have a speaker on the main tier.
    let mut saw_speaker = false;
    for utt in chat_file.utterances() {
        let speaker = format!("{}", utt.main.speaker);
        assert!(!speaker.is_empty(), "speaker code should render non-empty");
        saw_speaker = true;
    }
    assert!(
        saw_speaker,
        "minimal sample should contain at least one utterance"
    );
    Ok(())
}

/// Book example: "Parsing and Validating a CHAT File" (batch with reused parser).
#[test]
fn book_parsing_and_validating_with_parser() -> Result<(), Box<dyn std::error::Error>> {
    let parser = TreeSitterParser::new()?;
    let options = ParseValidateOptions::default().with_validation();
    let chat_files: Vec<&str> = vec![SAMPLE_MINIMAL, SAMPLE_WITH_MOR];

    for source in &chat_files {
        let chat_file = parse_and_validate_with_parser(&parser, source, options.clone())?;
        let _ = chat_file;
    }
    Ok(())
}

/// Book example: "Working with the Model" — top-level participants,
/// dependent-tier iteration, MorTier::items() accessor.
#[test]
fn book_working_with_the_model() -> Result<(), PipelineError> {
    let chat_file = parse_and_validate(
        SAMPLE_WITH_MOR,
        ParseValidateOptions::default().with_validation(),
    )?;

    let _participants = &chat_file.participants;
    assert!(
        !chat_file.participants.is_empty(),
        "sample should yield at least one participant"
    );

    let mut saw_mor_item = false;
    for utt in chat_file.utterances() {
        for tier in &utt.dependent_tiers {
            if let DependentTier::Mor(mor_tier) = tier {
                for item in mor_tier.items() {
                    let _pos = format!("{}", item.main.pos);
                    let _lemma = format!("{}", item.main.lemma);
                    saw_mor_item = true;
                }
            }
        }
    }
    assert!(saw_mor_item, "SAMPLE_WITH_MOR should expose mor items");
    Ok(())
}

/// Book example: "Serializing to CHAT" — `WriteChat::to_chat_string()`
/// convenience + the streaming `write_chat(&mut output)` form.
#[test]
fn book_serializing_to_chat() -> Result<(), Box<dyn std::error::Error>> {
    let chat_file = parse_and_validate(
        SAMPLE_MINIMAL,
        ParseValidateOptions::default().with_validation(),
    )?;

    let chat_text = chat_file.to_chat_string();
    assert!(chat_text.starts_with("@UTF8"));

    let mut output = String::new();
    chat_file.write_chat(&mut output)?;
    assert!(output.starts_with("@UTF8"));
    Ok(())
}

/// Book example: "Serializing to JSON" via the schema-validated helper.
#[test]
fn book_serializing_to_json() -> Result<(), Box<dyn std::error::Error>> {
    let chat_file = parse_and_validate(
        SAMPLE_MINIMAL,
        ParseValidateOptions::default().with_validation(),
    )?;
    let json = to_json_pretty_validated(&chat_file)?;
    assert!(json.contains("\"speaker\""));
    Ok(())
}

/// Book example: "Custom Error Handling" — implementing the
/// `ErrorSink` trait against the published `ParseError` shape.
#[test]
fn book_custom_error_handling() {
    struct MyErrorHandler;

    impl ErrorSink for MyErrorHandler {
        fn report(&self, _error: ParseError) {
            // Book example formats with eprintln! — here we just
            // verify the field types compile against the trait.
        }
    }

    let _: &dyn ErrorSink = &MyErrorHandler;
}
