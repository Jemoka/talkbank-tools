//! Scoped semantic search for `talkbank/scopedFind`.

use std::collections::HashSet;

use regex::Regex;
use serde::Serialize;
use talkbank_model::model::{ChatFile, Line};

use crate::backend::execute_commands::ScopedFindRequest;
use crate::backend::state::Backend;

use super::line_index::LineIndex;
use super::shared::get_document_and_chat_file;

/// One scoped-find match returned to the extension.
#[derive(Serialize)]
struct ScopedFindMatch {
    /// Zero-based line number of the match.
    line: usize,
    /// Zero-based character offset on the line.
    character: usize,
    /// Match length in characters.
    length: usize,
    /// Tier label such as `main` or `mor`.
    tier: String,
    /// Speaker code for the containing utterance.
    speaker: String,
    /// Source line text used in pickers.
    line_text: String,
}

/// Handle `talkbank/scopedFind`.
pub(crate) fn handle_scoped_find(
    backend: &Backend,
    request: &ScopedFindRequest,
) -> Result<serde_json::Value, crate::backend::LspBackendError> {
    let (text, chat_file) = get_document_and_chat_file(backend, &request.uri)?;
    let matcher = build_matcher(&request.query, request.regex)?;
    let speaker_filter: HashSet<&str> = request
        .speakers
        .iter()
        .map(|speaker| speaker.as_str())
        .collect();
    let line_index = LineIndex::new(&text);
    let lines: Vec<&str> = text.lines().collect();

    let matches = find_in_scoped_tiers(
        &chat_file,
        &text,
        &lines,
        &line_index,
        &request.scope,
        &speaker_filter,
        &matcher,
    );

    serde_json::to_value(&matches).map_err(Into::into)
}

/// One matching strategy for scoped-find.
pub(super) enum Matcher {
    /// Case-insensitive plain substring search.
    Plain(String),
    /// Regex-based search.
    Regex(Regex),
}

/// Build a plain-text or regex matcher from user input.
fn build_matcher(query: &str, is_regex: bool) -> Result<Matcher, crate::backend::LspBackendError> {
    if is_regex {
        Regex::new(query).map(Matcher::Regex).map_err(|e| {
            crate::backend::LspBackendError::InvalidRegex {
                reason: e.to_string(),
            }
        })
    } else {
        Ok(Matcher::Plain(query.to_lowercase()))
    }
}

/// Find all matching spans inside one byte range.
fn find_matches_in_span(
    text: &str,
    start: usize,
    end: usize,
    matcher: &Matcher,
) -> Vec<(usize, usize)> {
    let end = end.min(text.len());
    if start >= end {
        return vec![];
    }
    let slice = &text[start..end];
    let mut hits: Vec<(usize, usize)> = Vec::new();

    match matcher {
        Matcher::Plain(query_lower) => {
            let slice_lower = slice.to_lowercase();
            let mut search_from = 0;
            while let Some(pos) = slice_lower[search_from..].find(query_lower.as_str()) {
                let abs = start + search_from + pos;
                hits.push((abs, query_lower.len()));
                search_from += pos + 1;
            }
        }
        Matcher::Regex(re) => {
            for m in re.find_iter(slice) {
                hits.push((start + m.start(), m.len()));
            }
        }
    }

    hits
}

/// Collect all tier matches for the requested scope and speaker filter.
fn find_in_scoped_tiers(
    chat_file: &ChatFile,
    text: &str,
    lines: &[&str],
    line_index: &LineIndex,
    scope: &str,
    speaker_filter: &HashSet<&str>,
    matcher: &Matcher,
) -> Vec<ScopedFindMatch> {
    let mut results = Vec::new();
    let search_main = scope == "main" || scope == "all";
    let search_dependent = scope != "main";

    for model_line in &chat_file.lines {
        let Line::Utterance(utterance) = model_line else {
            continue;
        };

        let speaker = utterance.main.speaker.as_str();
        if !speaker_filter.is_empty() && !speaker_filter.contains(speaker) {
            continue;
        }

        if search_main {
            let span = utterance.main.span;
            if !span.is_dummy() {
                let hits =
                    find_matches_in_span(text, span.start as usize, span.end as usize, matcher);
                for (byte_offset, match_len) in hits {
                    if let Some(m) = make_match(
                        byte_offset,
                        match_len,
                        "main",
                        speaker,
                        line_index,
                        lines,
                        text,
                    ) {
                        results.push(m);
                    }
                }
            }
        }

        if search_dependent {
            for dt in &utterance.dependent_tiers {
                let tier_label = dt.kind();
                if scope != "all" && tier_label != scope {
                    continue;
                }

                let span = dt.span();
                if span.is_dummy() {
                    continue;
                }

                let hits =
                    find_matches_in_span(text, span.start as usize, span.end as usize, matcher);
                for (byte_offset, match_len) in hits {
                    if let Some(m) = make_match(
                        byte_offset,
                        match_len,
                        tier_label,
                        speaker,
                        line_index,
                        lines,
                        text,
                    ) {
                        results.push(m);
                    }
                }
            }
        }
    }

    results
}

/// Convert one byte-span hit into the response payload expected by the UI.
fn make_match(
    byte_offset: usize,
    match_len: usize,
    tier: &str,
    speaker: &str,
    line_index: &LineIndex,
    lines: &[&str],
    text: &str,
) -> Option<ScopedFindMatch> {
    let line = line_index.byte_to_line(byte_offset);
    let line_start = line_index.line_start(line)?;
    let char_offset = text[line_start..byte_offset].chars().count();
    let char_len = text[byte_offset..byte_offset + match_len].chars().count();
    let line_text = lines.get(line).unwrap_or(&"").to_string();

    Some(ScopedFindMatch {
        line,
        character: char_offset,
        length: char_len,
        tier: tier.to_string(),
        speaker: speaker.to_string(),
        line_text,
    })
}

#[cfg(test)]
/// Test helper that exposes matcher construction without the full handler path.
pub(super) fn test_build_matcher(
    query: &str,
    is_regex: bool,
) -> Result<Matcher, crate::backend::LspBackendError> {
    build_matcher(query, is_regex)
}

#[cfg(test)]
/// Test helper that exposes span matching without the full handler path.
pub(super) fn test_find_matches_in_span(
    text: &str,
    start: usize,
    end: usize,
    matcher: &Matcher,
) -> Vec<(usize, usize)> {
    find_matches_in_span(text, start, end, matcher)
}

#[cfg(test)]
/// Test helper that exposes match materialization without the full handler path.
pub(super) fn test_make_match(
    byte_offset: usize,
    match_len: usize,
    tier: &str,
    speaker: &str,
    line_index: &LineIndex,
    lines: &[&str],
    text: &str,
) -> Option<(usize, usize, usize, String, String)> {
    make_match(
        byte_offset,
        match_len,
        tier,
        speaker,
        line_index,
        lines,
        text,
    )
    .map(|m| (m.line, m.character, m.length, m.tier, m.speaker))
}
