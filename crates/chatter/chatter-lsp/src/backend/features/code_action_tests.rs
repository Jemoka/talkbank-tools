use super::*;

fn make_diagnostic(code: &str, message: &str, line: u32, start: u32, end: u32) -> Diagnostic {
    Diagnostic {
        range: Range {
            start: Position {
                line,
                character: start,
            },
            end: Position {
                line,
                character: end,
            },
        },
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String(code.to_string())),
        source: Some("talkbank".to_string()),
        message: message.to_string(),
        ..Default::default()
    }
}

/// Helper: extract the single text edit from a one-action result.
fn extract_edit(actions: &[CodeActionOrCommand], uri: &Url) -> TextEdit {
    assert_eq!(actions.len(), 1);
    match &actions[0] {
        CodeActionOrCommand::CodeAction(action) => {
            let edit = action.edit.as_ref().unwrap();
            let changes = edit.changes.as_ref().unwrap();
            changes[uri][0].clone()
        }
        _ => panic!("Expected CodeAction"),
    }
}

#[test]
fn test_fix_undeclared_speaker_adds_to_participants() {
    let uri = Url::parse("file:///test.cha").unwrap();
    let doc = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n*CHI:\thello .\n*FOO:\thi .\n@End\n";
    let diag = make_diagnostic(
        "E308",
        "Speaker 'FOO' is not in the participant list",
        5,
        1,
        4,
    );

    let actions = code_action(uri.clone(), vec![diag], Some(doc)).unwrap();
    let edit = extract_edit(&actions, &uri);
    assert_eq!(edit.new_text, ", FOO Participant");
    assert_eq!(edit.range.start.line, 3);
}

#[test]
fn test_fix_missing_end_inserts_at_eof() {
    let uri = Url::parse("file:///test.cha").unwrap();
    let doc = "@UTF8\n@Begin\n*CHI:\thello .\n";
    let diag = make_diagnostic("E502", "Missing @End", 0, 0, 0);

    let actions = code_action(uri.clone(), vec![diag], Some(doc)).unwrap();
    match &actions[0] {
        CodeActionOrCommand::CodeAction(action) => assert!(action.title.contains("@End")),
        _ => panic!("Expected CodeAction"),
    }
}

#[test]
fn test_fix_missing_utf8_inserts_at_start() {
    let uri = Url::parse("file:///test.cha").unwrap();
    let diag = make_diagnostic("E503", "Missing @UTF8", 0, 0, 0);

    let actions = code_action(uri.clone(), vec![diag], None).unwrap();
    let edit = extract_edit(&actions, &uri);
    assert_eq!(edit.range.start.line, 0);
    assert_eq!(edit.new_text, "@UTF8\n");
}

#[test]
fn test_fix_missing_begin_inserts_after_utf8() {
    let uri = Url::parse("file:///test.cha").unwrap();
    let doc = "@UTF8\n*CHI:\thello .\n@End\n";
    let diag = make_diagnostic("E501", "Missing @Begin", 0, 0, 0);

    let actions = code_action(uri.clone(), vec![diag], Some(doc)).unwrap();
    let edit = extract_edit(&actions, &uri);
    assert_eq!(edit.range.start.line, 1);
    assert_eq!(edit.new_text, "@Begin\n");
}

#[test]
fn test_fix_undeclared_speaker_no_participants_line() {
    let uri = Url::parse("file:///test.cha").unwrap();
    let doc = "@UTF8\n@Begin\n*FOO:\thello .\n@End\n";
    let diag = make_diagnostic(
        "E308",
        "Speaker 'FOO' is not in the participant list",
        2,
        1,
        4,
    );

    let actions = code_action(uri, vec![diag], Some(doc));
    assert!(actions.is_none());
}

#[test]
fn test_fix_timestamp_swap() {
    let uri = Url::parse("file:///test.cha").unwrap();
    let doc = "*CHI:\thello \u{2022}5000_1000\u{2022} .\n";
    let diag = make_diagnostic("E362", "Timestamp end before start", 0, 12, 23);

    let actions = code_action(uri.clone(), vec![diag], Some(doc)).unwrap();
    let edit = extract_edit(&actions, &uri);
    assert!(edit.new_text.contains("1000_5000"));
}

#[test]
fn test_fix_empty_utterance_deletes_line() {
    let uri = Url::parse("file:///test.cha").unwrap();
    let diag = make_diagnostic("E306", "Empty utterance", 2, 0, 5);

    let actions = code_action(uri.clone(), vec![diag], None).unwrap();
    let edit = extract_edit(&actions, &uri);
    assert_eq!(edit.new_text, "");
    assert_eq!(edit.range.start.line, 2);
    assert_eq!(edit.range.end.line, 3);
}

#[test]
fn test_fix_unclosed_bracket() {
    let uri = Url::parse("file:///test.cha").unwrap();
    let diag = make_diagnostic("E312", "Unclosed bracket", 2, 5, 15);

    let actions = code_action(uri.clone(), vec![diag], None).unwrap();
    let edit = extract_edit(&actions, &uri);
    assert_eq!(edit.new_text, "]");
    assert_eq!(edit.range.start, edit.range.end);
    assert_eq!(edit.range.start.character, 15);
}

#[test]
fn test_fix_unclosed_parenthesis() {
    let uri = Url::parse("file:///test.cha").unwrap();
    let diag = make_diagnostic("E313", "Unclosed parenthesis", 1, 8, 12);

    let actions = code_action(uri.clone(), vec![diag], None).unwrap();
    let edit = extract_edit(&actions, &uri);
    assert_eq!(edit.new_text, ")");
}

#[test]
fn test_fix_missing_colon_after_speaker() {
    let uri = Url::parse("file:///test.cha").unwrap();
    let diag = make_diagnostic("E323", "Missing colon after speaker", 3, 0, 4);

    let actions = code_action(uri.clone(), vec![diag], None).unwrap();
    let edit = extract_edit(&actions, &uri);
    assert_eq!(edit.new_text, ":");
    assert_eq!(edit.range.start.character, 4);
}

#[test]
fn test_fix_empty_participants_header() {
    let uri = Url::parse("file:///test.cha").unwrap();
    let diag = make_diagnostic("E506", "Empty @Participants header", 2, 0, 14);

    let actions = code_action(uri.clone(), vec![diag], None).unwrap();
    let edit = extract_edit(&actions, &uri);
    assert!(edit.new_text.contains("CHI"));
}

#[test]
fn test_fix_empty_languages_header() {
    let uri = Url::parse("file:///test.cha").unwrap();
    let diag = make_diagnostic("E507", "Empty @Languages header", 3, 0, 11);

    let actions = code_action(uri.clone(), vec![diag], None).unwrap();
    let edit = extract_edit(&actions, &uri);
    assert!(edit.new_text.contains("eng"));
}

#[test]
fn test_fix_consecutive_commas() {
    let uri = Url::parse("file:///test.cha").unwrap();
    let diag = make_diagnostic("E258", "Consecutive commas", 0, 10, 12);

    let actions = code_action(uri.clone(), vec![diag], None).unwrap();
    let edit = extract_edit(&actions, &uri);
    assert_eq!(edit.new_text, ",");
}

#[test]
fn test_fix_missing_terminator_offers_three_options() {
    let uri = Url::parse("file:///test.cha").unwrap();
    let diag = make_diagnostic("E301", "Missing terminator", 1, 0, 10);

    let actions = code_action(uri.clone(), vec![diag], None).unwrap();
    assert_eq!(actions.len(), 3); // ., ?, !
}

#[test]
fn test_no_actions_for_unknown_code() {
    let uri = Url::parse("file:///test.cha").unwrap();
    let diag = make_diagnostic("E999", "Unknown error", 0, 0, 5);

    let actions = code_action(uri, vec![diag], None);
    assert!(actions.is_none());
}

#[test]
fn test_fix_orphaned_gra_deletes_line() {
    let uri = Url::parse("file:///test.cha").unwrap();
    let diag = make_diagnostic("E604", "%gra without %mor", 5, 0, 20);

    let actions = code_action(uri.clone(), vec![diag], None).unwrap();
    let edit = extract_edit(&actions, &uri);
    assert_eq!(edit.new_text, "");
    assert_eq!(edit.range.start.line, 5);
    assert_eq!(edit.range.end.line, 6);
}

#[test]
fn test_fix_comma_after_non_spoken_removes_it() {
    let uri = Url::parse("file:///test.cha").unwrap();
    let diag = make_diagnostic("E259", "Comma after non-spoken", 0, 15, 16);

    let actions = code_action(uri.clone(), vec![diag], None).unwrap();
    let edit = extract_edit(&actions, &uri);
    assert_eq!(edit.new_text, "");
}

#[test]
fn test_fix_e504_participants_only() {
    let uri = Url::parse("file:///test.cha").unwrap();
    let doc = "@UTF8\n@Begin\n*CHI:\thello .\n@End\n";
    let diag = make_diagnostic("E504", "Missing required header: @Participants", 0, 0, 0);

    let actions = code_action(uri.clone(), vec![diag], Some(doc)).unwrap();
    let edit = extract_edit(&actions, &uri);
    assert!(edit.new_text.contains("@Participants:"));
}

#[test]
fn test_fix_e504_non_participants_ignored() {
    let uri = Url::parse("file:///test.cha").unwrap();
    let doc = "@UTF8\n@Begin\n*CHI:\thello .\n@End\n";
    let diag = make_diagnostic("E504", "Missing required header: @Languages", 0, 0, 0);

    let actions = code_action(uri, vec![diag], Some(doc));
    assert!(actions.is_none());
}
