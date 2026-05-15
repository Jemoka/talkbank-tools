use tower_lsp::lsp_types::*;

use super::builders::{
    delete_diagnostic_line, document_end_position, insert_at, insert_at_diagnostic_end,
    replace_diagnostic_range,
};

pub(super) fn actions_for_diagnostic(
    uri: &Url,
    diagnostic: &Diagnostic,
    doc: Option<&str>,
) -> Vec<CodeAction> {
    let Some(code) = diagnostic_code(diagnostic) else {
        return Vec::new();
    };

    match code {
        "E241" => vec![replace_diagnostic_range(
            uri,
            diagnostic,
            "xxx",
            "Replace 'xx' with 'xxx' (proper untranscribed marker)",
        )],
        "E242" => vec![insert_at(
            uri,
            diagnostic.range.end,
            " +...",
            "Add '+...' (trailing off marker)",
            Some(diagnostic),
        )],
        "E301" | "E305" => missing_terminator_actions(uri, diagnostic),
        "E308" => doc
            .and_then(|text| undeclared_speaker_action(uri, diagnostic, text))
            .into_iter()
            .collect(),
        "E502" => doc
            .map(|text| insert_at_end(uri, text, "@End\n", "Insert '@End' at end of file"))
            .into_iter()
            .collect(),
        "E503" => vec![insert_at_start(
            uri,
            "@UTF8\n",
            "Insert '@UTF8' at start of file",
        )],
        "E501" => doc
            .and_then(|text| {
                insert_after_utf8(uri, text, "@Begin\n", "Insert '@Begin' after @UTF8")
            })
            .into_iter()
            .collect(),
        "E306" => vec![delete_diagnostic_line(
            uri,
            diagnostic,
            "Delete empty utterance",
        )],
        "E322" => vec![delete_diagnostic_line(
            uri,
            diagnostic,
            "Delete empty colon line",
        )],
        "E362" => doc
            .and_then(|text| timestamp_swap_action(uri, diagnostic, text))
            .into_iter()
            .collect(),
        "E504" => {
            if diagnostic.message.contains("Participants") {
                doc.and_then(|text| {
                    insert_after_utf8(
                        uri,
                        text,
                        "@Participants:\tCHI Child\n",
                        "Insert '@Participants:' after @Begin",
                    )
                })
                .into_iter()
                .collect()
            } else {
                Vec::new()
            }
        }
        "E604" => vec![delete_diagnostic_line(
            uri,
            diagnostic,
            "Delete orphaned '%gra' tier",
        )],
        "E258" => vec![replace_diagnostic_range(
            uri,
            diagnostic,
            ",",
            "Replace ',,' with ','",
        )],
        "E259" => vec![replace_diagnostic_range(
            uri,
            diagnostic,
            "",
            "Remove comma after non-spoken content",
        )],
        "E312" => vec![insert_at_diagnostic_end(
            uri,
            diagnostic,
            "]",
            "Add closing bracket ']'",
        )],
        "E313" => vec![insert_at_diagnostic_end(
            uri,
            diagnostic,
            ")",
            "Add closing parenthesis ')'",
        )],
        "E323" => vec![insert_at_diagnostic_end(
            uri,
            diagnostic,
            ":",
            "Add ':' after speaker code",
        )],
        "E244" => vec![replace_diagnostic_range(
            uri,
            diagnostic,
            "ˈ",
            "Replace consecutive stress markers with single 'ˈ'",
        )],
        "E506" => vec![replace_diagnostic_range(
            uri,
            diagnostic,
            "@Participants:\tCHI Child",
            "Insert participant template",
        )],
        "E507" => vec![replace_diagnostic_range(
            uri,
            diagnostic,
            "@Languages:\teng",
            "Insert language 'eng'",
        )],
        _ => Vec::new(),
    }
}

fn diagnostic_code(diagnostic: &Diagnostic) -> Option<&str> {
    match &diagnostic.code {
        Some(NumberOrString::String(code)) => Some(code.as_str()),
        _ => None,
    }
}

fn missing_terminator_actions(uri: &Url, diagnostic: &Diagnostic) -> Vec<CodeAction> {
    [
        (".", "Add '.' (declarative/default)"),
        ("?", "Add '?' (question)"),
        ("!", "Add '!' (exclamation)"),
    ]
    .into_iter()
    .map(|(terminator, title)| {
        insert_at(
            uri,
            diagnostic.range.end,
            format!(" {terminator}"),
            title,
            Some(diagnostic),
        )
    })
    .collect()
}

fn undeclared_speaker_action(uri: &Url, diagnostic: &Diagnostic, doc: &str) -> Option<CodeAction> {
    let speaker = speaker_code_from_message(&diagnostic.message)?;
    let (line_idx, line_text) = doc
        .lines()
        .enumerate()
        .find(|(_, line)| line.starts_with("@Participants:"))?;

    Some(insert_at(
        uri,
        Position {
            line: line_idx as u32,
            character: line_text.len() as u32,
        },
        format!(", {speaker} Participant"),
        format!("Add '{speaker}' to @Participants"),
        Some(diagnostic),
    ))
}

fn speaker_code_from_message(message: &str) -> Option<&str> {
    let start = message.find("Speaker '")? + "Speaker '".len();
    let end = start + message[start..].find('\'')?;
    Some(&message[start..end])
}

fn insert_at_end(uri: &Url, doc: &str, text: &str, title: &str) -> CodeAction {
    let insert_text = if doc.ends_with('\n') {
        text.to_string()
    } else {
        format!("\n{text}")
    };

    insert_at(uri, document_end_position(doc), insert_text, title, None)
}

fn insert_at_start(uri: &Url, text: &str, title: &str) -> CodeAction {
    insert_at(
        uri,
        Position {
            line: 0,
            character: 0,
        },
        text,
        title,
        None,
    )
}

fn insert_after_utf8(uri: &Url, doc: &str, text: &str, title: &str) -> Option<CodeAction> {
    let insert_line = doc
        .lines()
        .enumerate()
        .find(|(_, line)| line.starts_with("@UTF8"))
        .map(|(index, _)| index as u32 + 1)
        .unwrap_or(0);

    Some(insert_at(
        uri,
        Position {
            line: insert_line,
            character: 0,
        },
        text,
        title,
        None,
    ))
}

fn timestamp_swap_action(uri: &Url, diagnostic: &Diagnostic, doc: &str) -> Option<CodeAction> {
    let line = doc.lines().nth(diagnostic.range.start.line as usize)?;
    let start_char = diagnostic.range.start.character as usize;
    let end_char = diagnostic.range.end.character as usize;
    let start_byte = line
        .char_indices()
        .nth(start_char)
        .map(|(index, _)| index)?;
    let end_byte = line
        .char_indices()
        .nth(end_char)
        .map(|(index, _)| index)
        .unwrap_or(line.len());
    let bullet_text = &line[start_byte..end_byte];

    let inner = bullet_text
        .trim_start_matches('\u{2022}')
        .trim_end_matches('\u{2022}');
    let parts: Vec<&str> = inner.split('_').collect();
    if parts.len() != 2 {
        return None;
    }

    let swapped = format!("\u{2022}{}_{}\u{2022}", parts[1], parts[0]);
    Some(replace_diagnostic_range(
        uri,
        diagnostic,
        swapped.clone(),
        format!("Swap timestamps: {bullet_text} → {swapped}"),
    ))
}
