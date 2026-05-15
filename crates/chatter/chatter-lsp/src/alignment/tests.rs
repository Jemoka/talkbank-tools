//! Regression tests for alignment hover resolution across tier combinations.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use super::find_alignment_hover_info;
use talkbank_model::model::Line;
use talkbank_model::{ErrorCode, ErrorCollector};
use talkbank_parser::TreeSitterParser;
use tower_lsp::lsp_types::Position;

/// Helper to parse and validate a CHAT file
fn parse_and_validate_chat_file(
    content: &str,
) -> Result<(talkbank_model::model::ChatFile, tree_sitter::Tree), String> {
    let parser =
        TreeSitterParser::new().map_err(|err| format!("Failed to create parser: {err:?}"))?;

    // Parse the file
    let mut chat_file = parser
        .parse_chat_file(content)
        .map_err(|err| format!("Failed to parse CHAT file: {err:?}"))?;

    // Compute alignments for all utterances
    for line in &mut chat_file.lines {
        if let Line::Utterance(utterance) = line {
            utterance.compute_alignments_default();
        }
    }

    let tree = parser
        .parse_tree_incremental(content, None)
        .map_err(|err| format!("Failed to parse CST: {err:?}"))?;

    // Validate the file (but allow validation warnings - we only care about parse errors)
    let error_sink = ErrorCollector::new();
    chat_file.validate(&error_sink, None);
    let errors = error_sink.into_vec();

    // Only fatal parse errors should fail the test (not validation warnings)
    let fatal_errors: Vec<_> = errors
        .iter()
        .filter(|e| matches!(e.severity, talkbank_model::Severity::Error))
        .filter(|e| {
            matches!(
                e.code,
                ErrorCode::InternalError
                    | ErrorCode::TestError
                    | ErrorCode::EmptyString
                    | ErrorCode::InvalidLineFormat
            )
        })
        .collect();

    if !fatal_errors.is_empty() {
        return Err(format!("CHAT file has parse errors: {:?}", fatal_errors));
    }

    Ok((chat_file, tree))
}

/// Tests main tier hover shows mor alignment.
#[test]
fn test_main_tier_hover_shows_mor_alignment() -> Result<(), String> {
    // Use a simple example with %mor tier
    let content = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|test|CHI||||Target_Child|||
*CHI:	more cookie .
%mor:	qn|more n|cookie .
@End
"#;

    let (chat_file, tree) = parse_and_validate_chat_file(content)?;

    // Cursor on "more" (line 5, after "*CHI:\t")
    // Line 5 is "*CHI:\tmore cookie ."
    // Position after tab is character 6
    let position = Position {
        line: 5,
        character: 7, // On "more"
    };

    let hover_info = find_alignment_hover_info(&chat_file, &tree, position, content);

    assert!(
        hover_info.is_some(),
        "Should return hover info for main tier word"
    );

    let info = hover_info.ok_or_else(|| "Expected hover info, got None".to_string())?;
    assert_eq!(info.element_type, "Main Tier Word");
    assert!(info.aligned_to_mor.is_some(), "Should show %mor alignment");
    Ok(())
}

/// Tests mor tier hover shows main alignment.
#[test]
fn test_mor_tier_hover_shows_main_alignment() -> Result<(), String> {
    let content = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|test|CHI||||Target_Child|||
*CHI:	more cookie .
%mor:	qn|more n|cookie .
@End
"#;

    let (chat_file, tree) = parse_and_validate_chat_file(content)?;

    // Cursor on "%mor:\tqn|more" (line 6)
    let position = Position {
        line: 6,
        character: 7, // On "qn|more"
    };

    let hover_info = find_alignment_hover_info(&chat_file, &tree, position, content);

    assert!(
        hover_info.is_some(),
        "Should return hover info for %mor element"
    );

    let info = hover_info.ok_or_else(|| "Expected hover info, got None".to_string())?;
    assert_eq!(info.element_type, "Morphology Element");
    assert!(
        info.aligned_to_main.is_some(),
        "Should show main tier alignment"
    );
    Ok(())
}

/// Tests gra tier hover shows mor and main alignment.
#[test]
fn test_gra_tier_hover_shows_mor_and_main_alignment() -> Result<(), String> {
    let content = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|test|CHI||||Target_Child|||
*CHI:	more cookie .
%mor:	qn|more n|cookie .
%gra:	1|2|DET 2|0|ROOT 3|2|OBJ 4|2|PUNCT
@End
"#;

    let (chat_file, tree) = parse_and_validate_chat_file(content)?;

    // Cursor on "%gra:\t1|2|DET" (line 7)
    let position = Position {
        line: 7,
        character: 8,
    };

    let hover_info = find_alignment_hover_info(&chat_file, &tree, position, content);

    assert!(
        hover_info.is_some(),
        "Should return hover info for %gra element"
    );

    let info = hover_info.ok_or_else(|| "Expected hover info, got None".to_string())?;
    assert_eq!(info.element_type, "Grammatical Relation");
    assert!(info.aligned_to_mor.is_some());
    assert!(info.aligned_to_main.is_some());
    Ok(())
}

/// Tests non alignable tier returns none.
#[test]
fn test_non_alignable_tier_returns_none() -> Result<(), String> {
    let content = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|test|CHI||||Target_Child|||
*CHI:	more cookie .
%com:	this is a comment
@End
"#;

    let (chat_file, tree) = parse_and_validate_chat_file(content)?;

    let position = Position {
        line: 6,
        character: 10,
    };
    let hover_info = find_alignment_hover_info(&chat_file, &tree, position, content);

    assert!(
        hover_info.is_none(),
        "Non-alignable tiers should return None"
    );
    Ok(())
}

/// Tests non alignable content returns none.
#[test]
fn test_non_alignable_content_returns_none() -> Result<(), String> {
    let content = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|test|CHI||||Target_Child|||
*CHI:	more (.) cookie .
%mor:	qn|more n|cookie .
@End
"#;

    let (chat_file, tree) = parse_and_validate_chat_file(content)?;

    // Cursor on pause marker in main tier
    let position = Position {
        line: 5,
        character: 13,
    };
    let hover_info = find_alignment_hover_info(&chat_file, &tree, position, content);

    assert!(
        hover_info.is_none(),
        "Non-alignable content like pauses should return None"
    );
    Ok(())
}
