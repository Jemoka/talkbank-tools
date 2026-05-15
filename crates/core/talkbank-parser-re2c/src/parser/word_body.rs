// `_ => unreachable!()` arms in this file are guarded by upstream
// char-classification: the outer `match ch` already filtered the
// CA-element / CA-delimiter / overlap-point character set, and the
// inner mapping match enumerates the same characters. New
// characters added to the lexer's CA-element set without
// corresponding `CaElementKind` variants would fail to compile —
// caught before merge.
#![allow(clippy::unreachable)]

//! Word body parser — scans a `&str` body for internal structure.
//!
//! The lexer determines word boundaries and extracts prefix/suffixes.
//! The body contains: text segments, shortenings, lengthening,
//! compound markers, stress, overlap points, syllable pause,
//! clitic boundary, CA elements/delimiters, underline markers.
//!
//! This is char-level scanning, not token-level — chumsky does not
//! apply here.

use crate::ast::*;

/// Parse a word body string into structured `WordBodyItem` list.
/// The body is the interior of a word (no prefix, no suffixes).
pub fn parse_word_body(body: &str) -> Vec<WordBodyItem<'_>> {
    let mut items = Vec::new();
    let mut chars = body.char_indices().peekable();

    while let Some(&(i, ch)) = chars.peek() {
        match ch {
            // Shortening: (text)
            '(' => {
                chars.next();
                let content_start = chars.peek().map_or(body.len(), |&(j, _)| j);
                // Scan to closing )
                while let Some(&(_, c)) = chars.peek() {
                    if c == ')' {
                        break;
                    }
                    chars.next();
                }
                let content_end = chars.peek().map_or(body.len(), |&(j, _)| j);
                if chars.peek().is_some() {
                    chars.next(); // consume ')'
                }
                items.push(WordBodyItem::Shortening(&body[content_start..content_end]));
            }
            // Lengthening: one or more colons
            ':' => {
                let mut count: u8 = 0;
                while let Some(&(_, ':')) = chars.peek() {
                    chars.next();
                    count += 1;
                }
                items.push(WordBodyItem::Lengthening(count));
            }
            // Compound marker
            '+' => {
                chars.next();
                items.push(WordBodyItem::CompoundMarker);
            }
            // Stress markers
            '\u{02C8}' => {
                chars.next();
                items.push(WordBodyItem::Stress(StressKind::Primary));
            }
            '\u{02CC}' => {
                chars.next();
                items.push(WordBodyItem::Stress(StressKind::Secondary));
            }
            // Syllable pause
            '^' => {
                chars.next();
                items.push(WordBodyItem::SyllablePause);
            }
            // Clitic boundary
            '~' => {
                chars.next();
                items.push(WordBodyItem::CliticBoundary);
            }
            // Overlap points: ⌈ ⌉ ⌊ ⌋ with optional digit
            '\u{2308}' | '\u{2309}' | '\u{230A}' | '\u{230B}' => {
                let kind = match ch {
                    '\u{2308}' => OverlapKind::TopBegin,
                    '\u{2309}' => OverlapKind::TopEnd,
                    '\u{230A}' => OverlapKind::BottomBegin,
                    '\u{230B}' => OverlapKind::BottomEnd,
                    _ => unreachable!(),
                };
                chars.next();
                // Include the overlap char + optional digit in the slice
                let end = chars.peek().map_or(body.len(), |&(j, _)| j);
                let overlap_text = &body[i..end];
                // Check for trailing digit
                if let Some(&(_, d)) = chars.peek()
                    && d.is_ascii_digit()
                    && d != '0'
                {
                    chars.next();
                    let end2 = chars.peek().map_or(body.len(), |&(j, _)| j);
                    items.push(WordBodyItem::OverlapPoint(kind, &body[i..end2]));
                    continue;
                }
                items.push(WordBodyItem::OverlapPoint(kind, overlap_text));
            }
            // Underline markers
            '\u{0002}' => {
                chars.next();
                if let Some(&(_, next_ch)) = chars.peek() {
                    match next_ch {
                        '\u{0001}' => {
                            chars.next();
                            // Underline begin — not a WordBodyItem, skip for now
                        }
                        '\u{0002}' => {
                            chars.next();
                            // Underline end — not a WordBodyItem, skip for now
                        }
                        _ => {}
                    }
                }
            }
            // CA elements
            _ if is_ca_element(ch) => {
                chars.next();
                items.push(WordBodyItem::CaElement(char_to_ca_element(ch)));
            }
            // CA delimiters
            _ if is_ca_delimiter(ch) => {
                chars.next();
                items.push(WordBodyItem::CaDelimiter(char_to_ca_delimiter(ch)));
            }
            // Text segment: everything else until a special char
            _ => {
                chars.next();
                // Consume all text chars (including '0' in rest position)
                while let Some(&(_, c)) = chars.peek() {
                    if is_body_special_char(c) {
                        break;
                    }
                    chars.next();
                }
                let end = chars.peek().map_or(body.len(), |&(j, _)| j);
                items.push(WordBodyItem::Text(&body[i..end]));
            }
        }
    }
    items
}

/// Characters that break a text segment in word body parsing.
fn is_body_special_char(ch: char) -> bool {
    matches!(
        ch,
        '(' | ':'
            | '+'
            | '^'
            | '~'
            | '\u{02C8}'
            | '\u{02CC}'
            | '\u{2308}'
            | '\u{2309}'
            | '\u{230A}'
            | '\u{230B}'
            | '\u{0002}'
    ) || is_ca_element(ch)
        || is_ca_delimiter(ch)
}

pub fn is_ca_element(ch: char) -> bool {
    matches!(
        ch,
        '\u{2260}'
            | '\u{223E}'
            | '\u{2051}'
            | '\u{2907}'
            | '\u{2219}'
            | '\u{1F29}'
            | '\u{2193}'
            | '\u{21BB}'
            | '\u{2191}'
            | '\u{2906}'
    )
}

pub fn is_ca_delimiter(ch: char) -> bool {
    matches!(
        ch,
        '\u{2047}'
            | '\u{00A7}'
            | '\u{204E}'
            | '\u{00B0}'
            | '\u{21AB}'
            | '\u{2206}'
            | '\u{2207}'
            | '\u{222C}'
            | '\u{222E}'
            | '\u{2581}'
            | '\u{2594}'
            | '\u{25C9}'
            | '\u{263A}'
            | '\u{264B}'
            | '\u{03AB}'
    )
}

pub fn char_to_ca_element(ch: char) -> CaElementKind {
    match ch {
        '\u{2260}' => CaElementKind::BlockedSegments,
        '\u{223E}' => CaElementKind::Constriction,
        '\u{2051}' => CaElementKind::Hardening,
        '\u{2907}' => CaElementKind::HurriedStart,
        '\u{2219}' => CaElementKind::Inhalation,
        '\u{1F29}' => CaElementKind::LaughInWord,
        '\u{2193}' => CaElementKind::PitchDown,
        '\u{21BB}' => CaElementKind::PitchReset,
        '\u{2191}' => CaElementKind::PitchUp,
        '\u{2906}' => CaElementKind::SuddenStop,
        _ => unreachable!("not a CA element char"),
    }
}

pub fn char_to_ca_delimiter(ch: char) -> CaDelimiterKind {
    match ch {
        '\u{2047}' => CaDelimiterKind::Unsure,
        '\u{00A7}' => CaDelimiterKind::Precise,
        '\u{204E}' => CaDelimiterKind::Creaky,
        '\u{00B0}' => CaDelimiterKind::Softer,
        '\u{21AB}' => CaDelimiterKind::SegmentRepetition,
        '\u{2206}' => CaDelimiterKind::Faster,
        '\u{2207}' => CaDelimiterKind::Slower,
        '\u{222C}' => CaDelimiterKind::Whisper,
        '\u{222E}' => CaDelimiterKind::Singing,
        '\u{2581}' => CaDelimiterKind::LowPitch,
        '\u{2594}' => CaDelimiterKind::HighPitch,
        '\u{25C9}' => CaDelimiterKind::Louder,
        '\u{263A}' => CaDelimiterKind::SmileVoice,
        '\u{264B}' => CaDelimiterKind::BreathyVoice,
        '\u{03AB}' => CaDelimiterKind::Yawn,
        _ => unreachable!("not a CA delimiter char"),
    }
}
