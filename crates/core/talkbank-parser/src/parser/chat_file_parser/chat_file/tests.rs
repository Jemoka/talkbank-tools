//! Tests for this subsystem.
//!

use crate::error::ErrorCollector;
use crate::model::{LanguageCode, UtteranceContent, Word, WordCategory, WordContent};
use crate::parser::TreeSitterParser;
use crate::validation::{Validate, ValidationContext};

/// Parses chat file or err.
fn parse_chat_file_or_err(input: &str) -> Result<crate::model::ChatFile, String> {
    let parser =
        TreeSitterParser::new().map_err(|err| format!("Failed to create parser: {err}"))?;
    parser
        .parse_chat_file(input)
        .map_err(|err| format!("Failed to parse chat file: {err:?}"))
}

/// `@Languages: cat, spa` produces `ChatFile.languages = [cat, spa]`
/// in declaration order.
#[test]
fn cat_spa_header_parses_to_two_declared_languages() -> Result<(), String> {
    let input = "@UTF8\n\
                 @Begin\n\
                 @Languages:\tcat, spa\n\
                 @Participants:\tPAR Participant\n\
                 @ID:\tcat|test|PAR||male|||Participant|||\n\
                 *PAR:\thello .\n\
                 @End";
    let chat_file = parse_chat_file_or_err(input)?;

    let langs: Vec<&str> = chat_file.languages.0.iter().map(|c| c.as_str()).collect();

    assert_eq!(langs, vec!["cat", "spa"]);
    Ok(())
}

/// Tests shortening at word start.
#[test]
fn test_shortening_at_word_start() -> Result<(), String> {
    let input = "@UTF8\n@Begin\n*CHI:\t(th)at .\n@End";
    let result = parse_chat_file_or_err(input)?;

    let utterance = result
        .utterances()
        .next()
        .ok_or_else(|| "Expected an utterance".to_string())?;
    let main_tier = &utterance.main;

    // Should be parsed as ONE word "(th)at", not two words "(th)" and "at"
    assert_eq!(
        main_tier.content.content.len(),
        1,
        "GRAMMAR BUG: Should have exactly 1 word, not 2. Currently tree-sitter splits '(th)at' into '(th)' and 'at'"
    );

    let word = match &main_tier.content.content[0] {
        UtteranceContent::Word(w) => w,
        other => return Err(format!("Expected Word, got {:?}", other)),
    };

    // Verify the word structure
    assert_eq!(word.raw_text(), "(th)at", "Raw text should be '(th)at'");
    assert_eq!(word.cleaned_text(), "that", "Cleaned text should be 'that'");

    // Verify content structure: should be [Shortening("th"), Text("at")]
    assert_eq!(word.content.len(), 2, "Word should have 2 content items");

    if let WordContent::Shortening(text) = &word.content[0] {
        assert_eq!(text.as_ref(), "th", "First item should be shortening 'th'");
    } else {
        return Err(format!("Expected Shortening, got {:?}", &word.content[0]));
    }

    if let WordContent::Text(text) = &word.content[1] {
        assert_eq!(text.as_ref(), "at", "Second item should be text 'at'");
    } else {
        return Err(format!("Expected Text, got {:?}", &word.content[1]));
    }

    Ok(())
}

/// Tests ca mode parenthetical becomes ca omission.
#[test]
fn test_ca_mode_parenthetical_becomes_ca_omission() -> Result<(), String> {
    let input = "@UTF8\n@Begin\n@Options:\tCA\n*CHI:\t(word) .\n@End";
    let result = parse_chat_file_or_err(input)?;

    let utterance = result
        .utterances()
        .next()
        .ok_or_else(|| "Expected an utterance".to_string())?;
    let main_tier = &utterance.main;

    let word = match &main_tier.content.content[0] {
        UtteranceContent::Word(w) => w,
        other => return Err(format!("Expected Word, got {:?}", other)),
    };

    assert!(
        matches!(word.category, Some(WordCategory::CAOmission)),
        "Expected CAOmission category for parenthetical in CA mode"
    );
    assert_eq!(
        word.content.len(),
        1,
        "CA omission should have text-only content"
    );

    if let WordContent::Text(text) = &word.content[0] {
        assert_eq!(text.as_ref(), "word", "Inner text should be plain text");
    } else {
        return Err(format!("Expected Text content, got {:?}", &word.content[0]));
    }

    assert_eq!(
        word.to_chat(),
        "(word)",
        "CA omission should roundtrip to '(word)'"
    );

    Ok(())
}

// Deleted: test_ca_mode_parenthetical_with_overlap_becomes_ca_omission
// Deleted: test_low_pitch_delimiter_not_swallowed_into_underlined_text
// Both depended on Chumsky direct parser CA delimiter handling (removed fa9623b).
// CA delimiter recognition from word_segment text needs reimplementation in the
// CST-based word parser when CA support is prioritized.

/// Tests shortening in middle.
#[test]
fn test_shortening_in_middle() -> Result<(), String> {
    let input = "@UTF8\n@Begin\n*CHI:\tan(d) .\n@End";
    let result = parse_chat_file_or_err(input)?;

    let utterance = result
        .utterances()
        .next()
        .ok_or_else(|| "Expected an utterance".to_string())?;
    let main_tier = &utterance.main;

    assert_eq!(
        main_tier.content.content.len(),
        1,
        "GRAMMAR BUG: Should have exactly 1 word 'an(d)', not multiple words"
    );

    let word = match &main_tier.content.content[0] {
        UtteranceContent::Word(w) => w,
        other => return Err(format!("Expected Word, got {:?}", other)),
    };

    assert_eq!(word.raw_text(), "an(d)", "Raw text should be 'an(d)'");
    assert_eq!(word.cleaned_text(), "and", "Cleaned text should be 'and'");

    Ok(())
}

/// Tests prosodic marker followed by text.
#[test]
fn test_prosodic_marker_followed_by_text() -> Result<(), String> {
    let input = "@UTF8\n@Begin\n*CHI:\tm:hm .\n@End";
    let result = parse_chat_file_or_err(input)?;

    let utterance = result
        .utterances()
        .next()
        .ok_or_else(|| "Expected an utterance".to_string())?;
    let main_tier = &utterance.main;

    assert_eq!(
        main_tier.content.content.len(),
        1,
        "GRAMMAR BUG: Should have exactly 1 word 'm:hm', not multiple words"
    );

    let word = match &main_tier.content.content[0] {
        UtteranceContent::Word(w) => w,
        other => return Err(format!("Expected Word, got {:?}", other)),
    };

    assert_eq!(word.raw_text(), "m:hm", "Raw text should be 'm:hm'");

    // Verify roundtrip: serialization should preserve the prosodic marker
    let serialized = word.to_chat();
    assert_eq!(serialized, "m:hm", "Serialized form should preserve 'm:hm'");

    Ok(())
}

/// Tests main tier span is set.
#[test]
fn test_main_tier_span_is_set() -> Result<(), String> {
    let content = "@UTF8\n@Begin\n*CHI:\thello world\n@End\n";
    let chat_file = parse_chat_file_or_err(content)?;

    // Get the first main tier
    let utterance = chat_file
        .utterances()
        .next()
        .ok_or_else(|| "Expected an utterance".to_string())?;
    let main_tier = &utterance.main;

    // Check that span is not DUMMY (0..0)
    assert_ne!(main_tier.span.start, 0, "Span start should not be 0");
    assert_ne!(main_tier.span.end, 0, "Span end should not be 0");
    assert!(
        main_tier.span.end > main_tier.span.start,
        "Span should be non-empty"
    );

    // The span should cover "*CHI:\thello world\n"
    let span_text = &content[main_tier.span.start as usize..main_tier.span.end as usize];
    assert_eq!(
        span_text.get(0..5),
        Some("*CHI:"),
        "Span should start with *CHI:"
    );

    println!(
        "MainTier span: {}..{}",
        main_tier.span.start, main_tier.span.end
    );
    println!("MainTier text: {:?}", span_text);

    Ok(())
}

/// Tests validation error has proper span.
#[test]
fn test_validation_error_has_proper_span() -> Result<(), String> {
    let content = "@UTF8\n@Begin\n*CHI:\thello world\n@End\n";
    let chat_file = parse_chat_file_or_err(content)?;

    // Get the first main tier (missing terminator)
    let utterance = chat_file
        .utterances()
        .next()
        .ok_or_else(|| "Expected an utterance".to_string())?;
    let main_tier = &utterance.main;

    // Validate it
    let languages = vec![LanguageCode::new("eng")];
    let ctx = ValidationContext::new()
        .with_default_language(languages[0].clone())
        .with_declared_languages(languages);
    let error_sink = ErrorCollector::new();
    main_tier.validate(&ctx, &error_sink);
    let errors = error_sink.into_vec();

    // Should have validation errors (missing terminator)
    assert!(!errors.is_empty(), "Should have validation errors");

    // Check that error spans are not DUMMY
    for error in &errors {
        println!(
            "Error: {} at span {}..{}",
            error.message, error.location.span.start, error.location.span.end
        );
        if error.code.as_str() == "E304" {
            // Missing terminator error should have the main tier's span
            assert_ne!(
                error.location.span.start, 0,
                "E304 error should have non-zero start"
            );
            assert_ne!(
                error.location.span.end, 0,
                "E304 error should have non-zero end"
            );
            assert!(
                error.location.span.end > error.location.span.start,
                "E304 error span should be non-empty"
            );
        }
    }

    Ok(())
}
