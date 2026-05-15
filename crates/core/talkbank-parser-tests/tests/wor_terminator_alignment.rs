//! Tests for %wor tier parsing, alignment, and separator handling.
//!
//! Covers:
//! - Terminators are NOT counted as alignable content
//! - Tag-marker separators (comma `,`, tag `„`, vocative `‡`) parse as
//!   `WorItem::Separator`, not as words
//! - `words()` iterator and `word_count()` exclude separators
//! - Alignment counting excludes separators
//! - Roundtrip serialization preserves separators

use talkbank_model::alignment::{
    WorTimingSidecar,
    helpers::{TierDomain, count_tier_positions},
    resolve_wor_timing_sidecar,
};
use talkbank_model::model::dependent_tier::WorTier;
use talkbank_model::model::dependent_tier::wor::WorItem;
use talkbank_model::model::{Line, Terminator, WriteChat};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::test_error::TestError;

/// Tests wor tier terminator not counted in real parse.
#[test]
fn test_wor_tier_terminator_not_counted_in_real_parse() -> Result<(), TestError> {
    // Main tier: 3 words + 1 terminator
    // %wor tier: 3 words with timing bullets + terminator
    // Terminators are NOT counted as alignable content
    let source = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello world today .\n%wor:\thello \u{0015}100_200\u{0015} world \u{0015}300_400\u{0015} today \u{0015}500_600\u{0015} .\n@End\n";

    let parser = TreeSitterParser::new().map_err(|err| TestError::ParserInit(err.to_string()))?;
    let chat_file = parser.parse_chat_file(source)?;

    let line = chat_file
        .lines
        .iter()
        .find(|l| matches!(l, Line::Utterance(_)))
        .ok_or_else(|| TestError::Failure("No utterance found".to_string()))?;
    let utterance = match line {
        Line::Utterance(utt) => utt,
        _ => {
            return Err(TestError::Failure(
                "Expected utterance, got header".to_string(),
            ));
        }
    };

    let main = &utterance.main;
    let wor = utterance
        .wor_tier()
        .ok_or_else(|| TestError::Failure("No wor tier found".to_string()))?;

    // Debug: main tier
    println!("=== Main Tier ===");
    println!("Content items: {}", main.content.content.0.len());
    println!("Terminator: {:?}", main.content.terminator);
    println!(
        "Total alignable (for Wor domain): {}",
        count_tier_positions(&main.content.content, TierDomain::Wor)
    );

    // Debug: wor tier (flat items)
    println!("\n=== Wor Tier ===");
    println!("Word count: {}", wor.word_count());
    println!("Terminator: {:?}", wor.terminator);

    // Resolve the timing sidecar: 3 main-tier Wor words ↔ 3 %wor words.
    let sidecar = resolve_wor_timing_sidecar(main, wor);

    assert_eq!(
        sidecar,
        WorTimingSidecar::Positional { count: 3 },
        "terminator must not participate in the Wor filtered count"
    );

    Ok(())
}

/// Helper to extract the WorTier from a parsed CHAT string
fn parse_wor_tier(source: &str) -> Result<WorTier, TestError> {
    let parser = TreeSitterParser::new().map_err(|err| TestError::ParserInit(err.to_string()))?;
    let chat_file = parser.parse_chat_file(source)?;

    let utterance = chat_file
        .lines
        .iter()
        .find_map(|l| match l {
            Line::Utterance(utt) => Some(utt),
            _ => None,
        })
        .ok_or_else(|| TestError::Failure("No utterance found".to_string()))?;

    utterance
        .wor_tier()
        .cloned()
        .ok_or_else(|| TestError::Failure("No wor tier found".to_string()))
}

/// Tests wor terminator only.
#[test]
fn test_wor_terminator_only() -> Result<(), TestError> {
    // %wor:\t. — valid when main tier has only paralinguistic markers like &=nods
    let source = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\t&=nods .\n%wor:\t.\n@End\n";

    let wor = parse_wor_tier(source)?;

    // Should have 0 words and a period terminator
    assert_eq!(wor.word_count(), 0, "Expected 0 words");
    assert!(
        matches!(wor.terminator, Some(Terminator::Period { .. })),
        "Expected Period terminator, got {:?}",
        wor.terminator
    );

    Ok(())
}

/// Tests wor words with timing.
#[test]
fn test_wor_words_with_timing() -> Result<(), TestError> {
    // Normal %wor with words and timing bullets
    let source = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello world .\n%wor:\thello \u{0015}100_200\u{0015} world \u{0015}300_400\u{0015} .\n@End\n";

    let wor = parse_wor_tier(source)?;
    let words: Vec<_> = wor.words().collect();

    assert_eq!(words.len(), 2, "Expected 2 words");
    assert_eq!(words[0].cleaned_text(), "hello");
    let b0 = words[0]
        .inline_bullet
        .as_ref()
        .expect("expected bullet on word 0");
    assert_eq!(b0.timing.start_ms, 100);
    assert_eq!(b0.timing.end_ms, 200);

    assert_eq!(words[1].cleaned_text(), "world");
    let b1 = words[1]
        .inline_bullet
        .as_ref()
        .expect("expected bullet on word 1");
    assert_eq!(b1.timing.start_ms, 300);
    assert_eq!(b1.timing.end_ms, 400);
    assert!(matches!(wor.terminator, Some(Terminator::Period { .. })));

    Ok(())
}

/// Tests wor words without timing.
#[test]
fn test_wor_words_without_timing() -> Result<(), TestError> {
    // %wor with words but no timing bullets
    let source = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello world .\n%wor:\thello world .\n@End\n";

    let wor = parse_wor_tier(source)?;
    let words: Vec<_> = wor.words().collect();

    assert_eq!(words.len(), 2, "Expected 2 words");
    assert_eq!(words[0].cleaned_text(), "hello");
    assert!(words[0].inline_bullet.is_none());
    assert_eq!(words[1].cleaned_text(), "world");
    assert!(words[1].inline_bullet.is_none());
    assert!(matches!(wor.terminator, Some(Terminator::Period { .. })));

    Ok(())
}

/// Tests wor question terminator.
#[test]
fn test_wor_question_terminator() -> Result<(), TestError> {
    // %wor with question terminator
    let source = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\twhat ?\n%wor:\twhat ?\n@End\n";

    let wor = parse_wor_tier(source)?;
    let words: Vec<_> = wor.words().collect();

    assert_eq!(words.len(), 1);
    assert_eq!(words[0].cleaned_text(), "what");
    assert!(matches!(wor.terminator, Some(Terminator::Question { .. })));

    Ok(())
}

// ============================================================
// Separator regression tests
// ============================================================

/// Comma in %wor must parse as `WorItem::Separator`, NOT as a Word.
#[test]
fn test_wor_comma_parses_as_separator() -> Result<(), TestError> {
    // Real-world pattern: "he's in the water , too ."
    let source = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\twater , too .\n%wor:\twater \u{0015}100_200\u{0015} , too \u{0015}300_400\u{0015} .\n@End\n";

    let wor = parse_wor_tier(source)?;

    // Should have 3 items total: Word("water"), Separator(","), Word("too")
    assert_eq!(wor.items.len(), 3, "Expected 3 items (2 words + 1 comma)");

    assert!(
        matches!(&wor.items[0], WorItem::Word(w) if w.cleaned_text() == "water"),
        "First item should be Word('water'), got {:?}",
        wor.items[0]
    );
    assert!(
        matches!(&wor.items[1], WorItem::Separator { text, .. } if text == ","),
        "Second item should be Separator(','), got {:?}",
        wor.items[1]
    );
    assert!(
        matches!(&wor.items[2], WorItem::Word(w) if w.cleaned_text() == "too"),
        "Third item should be Word('too'), got {:?}",
        wor.items[2]
    );

    Ok(())
}

/// Tag marker `„` (U+201E) in %wor must parse as `WorItem::Separator`.
#[test]
fn test_wor_tag_marker_parses_as_separator() -> Result<(), TestError> {
    let source = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello \u{201E} world .\n%wor:\thello \u{0015}100_200\u{0015} \u{201E} world \u{0015}300_400\u{0015} .\n@End\n";

    let wor = parse_wor_tier(source)?;

    // 3 items: Word("hello"), Separator("„"), Word("world")
    assert_eq!(
        wor.items.len(),
        3,
        "Expected 3 items (2 words + 1 tag marker)"
    );

    assert!(matches!(&wor.items[0], WorItem::Word(w) if w.cleaned_text() == "hello"));
    assert!(
        matches!(&wor.items[1], WorItem::Separator { text, .. } if text == "\u{201E}"),
        "Expected tag marker separator, got {:?}",
        wor.items[1]
    );
    assert!(matches!(&wor.items[2], WorItem::Word(w) if w.cleaned_text() == "world"));

    Ok(())
}

/// Vocative marker `‡` (U+2021) in %wor must parse as `WorItem::Separator`.
#[test]
fn test_wor_vocative_marker_parses_as_separator() -> Result<(), TestError> {
    let source = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello \u{2021} world .\n%wor:\thello \u{0015}100_200\u{0015} \u{2021} world \u{0015}300_400\u{0015} .\n@End\n";

    let wor = parse_wor_tier(source)?;

    assert_eq!(
        wor.items.len(),
        3,
        "Expected 3 items (2 words + 1 vocative marker)"
    );

    assert!(matches!(&wor.items[0], WorItem::Word(w) if w.cleaned_text() == "hello"));
    assert!(
        matches!(&wor.items[1], WorItem::Separator { text, .. } if text == "\u{2021}"),
        "Expected vocative marker separator, got {:?}",
        wor.items[1]
    );
    assert!(matches!(&wor.items[2], WorItem::Word(w) if w.cleaned_text() == "world"));

    Ok(())
}

/// `words()` iterator must NOT yield separators.
#[test]
fn test_wor_words_iterator_excludes_separators() -> Result<(), TestError> {
    let source = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\twater , too .\n%wor:\twater \u{0015}100_200\u{0015} , too \u{0015}300_400\u{0015} .\n@End\n";

    let wor = parse_wor_tier(source)?;

    // items has 3 entries, but words() should yield only 2
    assert_eq!(wor.items.len(), 3);
    assert_eq!(
        wor.word_count(),
        2,
        "word_count() should exclude separators"
    );

    let words: Vec<_> = wor.words().collect();
    assert_eq!(words.len(), 2);
    assert_eq!(words[0].cleaned_text(), "water");
    assert_eq!(words[1].cleaned_text(), "too");

    Ok(())
}

/// Alignment must count only words, not separators.
/// Main tier has 2 words + comma, %wor has 2 words + comma separator.
/// Alignment should see 2 ↔ 2 and succeed.
#[test]
fn test_wor_alignment_excludes_separators() -> Result<(), TestError> {
    let source = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\twater , too .\n%wor:\twater \u{0015}100_200\u{0015} , too \u{0015}300_400\u{0015} .\n@End\n";

    let parser = TreeSitterParser::new().map_err(|err| TestError::ParserInit(err.to_string()))?;
    let chat_file = parser.parse_chat_file(source)?;

    let utterance = chat_file
        .lines
        .iter()
        .find_map(|l| match l {
            Line::Utterance(utt) => Some(utt),
            _ => None,
        })
        .ok_or_else(|| TestError::Failure("No utterance found".to_string()))?;

    let main = &utterance.main;
    let wor = utterance
        .wor_tier()
        .ok_or_else(|| TestError::Failure("No wor tier found".to_string()))?;

    // Main tier should have 2 alignable words (comma is a separator, not alignable for Wor)
    let main_count = count_tier_positions(&main.content.content, TierDomain::Wor);
    assert_eq!(
        main_count, 2,
        "Main tier should have 2 alignable words for Wor domain"
    );

    // Wor tier should have 2 words (comma separator excluded)
    assert_eq!(
        wor.word_count(),
        2,
        "Wor tier should have 2 words (separator excluded)"
    );

    // Separator-excluded counts match: 2 ↔ 2, so the sidecar is Positional.
    let sidecar = resolve_wor_timing_sidecar(main, wor);
    assert_eq!(sidecar, WorTimingSidecar::Positional { count: 2 });

    Ok(())
}

/// Roundtrip: %wor with comma must serialize back with comma in correct position.
#[test]
fn test_wor_comma_roundtrip_serialization() -> Result<(), TestError> {
    let source = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\twater , too .\n%wor:\twater \u{0015}100_200\u{0015} , too \u{0015}300_400\u{0015} .\n@End\n";

    let wor = parse_wor_tier(source)?;
    let serialized = wor.to_chat_string();

    // The serialized %wor should contain the comma between words
    assert!(
        serialized.contains(", too"),
        "Serialized %wor should contain ', too' but got: {:?}",
        serialized
    );
    assert!(
        serialized.contains("water"),
        "Serialized %wor should contain 'water' but got: {:?}",
        serialized
    );

    Ok(())
}
