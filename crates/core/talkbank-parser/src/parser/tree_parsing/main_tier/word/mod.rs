//! Word parsing from tree-sitter CST — direct CST traversal.
//!
//! The grammar parses word internals structurally:
//!   standalone_word = [word_prefix] word_body [form_marker] [word_lang_suffix] [pos_tag]
//!   word_body = repeat1(word_segment | shortening | stress_marker | lengthening | '+')
//!
//! This module walks the CST children to build the typed `Word` model
//! without any Chumsky dependency.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Words>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Word_Tier>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::model::Word;
use crate::parser::tree_parsing::parser_helpers::extract_utf8_text;
use smallvec::SmallVec;
use talkbank_model::ParseOutcome;
use talkbank_model::content::word::{
    FormType, WordCategory, WordCliticBoundary, WordCompoundMarker, WordContent, WordContents,
    WordLanguageMarker, WordLengthening, WordShortening, WordStressMarker, WordStressMarkerType,
    WordSyllablePause, WordText, WordUnderlineBegin, WordUnderlineEnd,
};
use talkbank_model::model::{LanguageCode, OverlapIndex, OverlapPoint, OverlapPointKind};
use tree_sitter::Node;

/// Convert a tree-sitter `standalone_word` node into the typed `Word` model.
///
/// Walks the CST children directly:
/// - `word_prefix` → `WordCategory`
/// - `word_body` children → `Vec<WordContent>`
/// - `form_marker` → `FormType`
/// - `word_lang_suffix` → `WordLanguageMarker`
/// - `pos_tag` → part of speech string
pub fn convert_word_node(node: Node, source: &str, errors: &impl ErrorSink) -> ParseOutcome<Word> {
    if node.is_missing() {
        errors.report(ParseError::new(
            ErrorCode::MalformedWordContent,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
            format!(
                "Internal error: attempted to convert MISSING tree-sitter node at byte {}",
                node.start_byte()
            ),
        ));
        return ParseOutcome::rejected();
    }

    let raw_text = extract_utf8_text(node, source, errors, "standalone_word", "");
    let span = talkbank_model::Span::from_usize(node.start_byte(), node.end_byte());

    let mut category = None;
    let mut content_items: SmallVec<[WordContent; 2]> = SmallVec::new();
    let mut form_type = None;
    let mut lang = None;
    let mut part_of_speech = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "word_prefix" => {
                let text = extract_utf8_text(child, source, errors, "word_prefix", "");
                category = match text {
                    "&-" => Some(WordCategory::Filler),
                    "&~" => Some(WordCategory::Nonword),
                    "&+" => Some(WordCategory::PhonologicalFragment),
                    _ => None,
                };
            }
            // Zero is inlined directly into standalone_word (not through word_prefix)
            // to resolve the tree-sitter shift-reduce conflict with nonword(zero).
            "zero" => {
                category = Some(WordCategory::Omission);
            }
            "word_body" => {
                build_word_contents(child, source, errors, &mut content_items);
            }
            "form_marker" => {
                let text = extract_utf8_text(child, source, errors, "form_marker", "");
                // text is like "@b" or "@z:grm" — may include :suffix from grammar
                // But the grammar captures form_marker as just "@b" (token.immediate),
                // and the :suffix is a separate child. Let's check both patterns.
                if let Some(ft) = FormType::parse(text) {
                    form_type = Some(ft);
                } else if text.starts_with("@z") {
                    // User-defined form: @z or @z:label
                    let label = text.strip_prefix("@z").unwrap_or("");
                    let label = label.strip_prefix(':').unwrap_or(label);
                    form_type = Some(FormType::UserDefined(label.to_string()));
                } else {
                    // Unknown form marker — report error (CHECK 147)
                    let marker = text.strip_prefix('@').unwrap_or(text);
                    errors.report(ParseError::new(
                        ErrorCode::InvalidFormType,
                        Severity::Error,
                        SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                        ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
                        format!("Undeclared form marker '@{}'", marker),
                    ).with_suggestion(
                        "Valid form markers: @b @c @d @f @fp @g @i @k @l @ls @n @o @p @q @sas @si @sl @t @u @wp @x @z"
                    ));
                    form_type = Some(FormType::UserDefined(marker.to_string()));
                }
            }
            "word_lang_suffix" => {
                lang = Some(build_lang_marker(child, source, errors));
            }
            "pos_tag" => {
                let text = extract_utf8_text(child, source, errors, "pos_tag", "");
                let tag = text.strip_prefix('$').unwrap_or(text);
                part_of_speech = Some(smol_str::SmolStr::from(tag));
            }
            _ => {
                // Skip unknown children (whitespace, etc.)
            }
        }
    }

    // If no content items were collected, use raw_text as a single Text item
    if content_items.is_empty()
        && let Some(wt) = WordText::new(raw_text)
    {
        content_items.push(WordContent::Text(wt));
    }

    // Compute cleaned_text from content items (Text + Shortening only)
    let cleaned: String = content_items
        .iter()
        .filter_map(|item| match item {
            WordContent::Text(t) => Some(t.as_ref()),
            WordContent::Shortening(s) => Some(s.as_ref()),
            _ => None,
        })
        .collect();

    // Guard: parser recovery can produce a word node with empty text (e.g.,
    // from `[: unclosed`). Report an error instead of panicking via
    // `new_unchecked`.
    if raw_text.is_empty() {
        errors.report(
            ParseError::new(
                ErrorCode::UnparsableContent,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
                "Unparsable content: word node produced empty text after parser recovery",
            )
            .with_suggestion(
                "Check for unclosed brackets or missing word content near this position",
            ),
        );
        return ParseOutcome::rejected();
    }

    // Guard: a word whose cleaned text is empty (e.g., only a stress marker
    // `ˈ` or only a lengthening marker `:`) would panic inside
    // `NonEmptyString::new_unchecked`. Emit E245 (stress marker not before
    // spoken material) for the lone-stress-marker case and reject the word
    // rather than fabricating a dummy `Word`.
    if cleaned.is_empty() {
        errors.report(
            ParseError::new(
                ErrorCode::StressNotBeforeSpokenMaterial,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
                format!(
                    "Word '{}' contains no spoken material after markers are stripped",
                    raw_text
                ),
            )
            .with_suggestion(
                "Stress and lengthening markers must attach to actual spoken material",
            ),
        );
        return ParseOutcome::rejected();
    }

    let mut word = Word::new_unchecked(raw_text, &cleaned);
    word.span = span;
    word.category = category;
    word.form_type = form_type;
    word.lang = lang;
    word.part_of_speech = part_of_speech;
    word.content = WordContents::new(content_items);

    ParseOutcome::parsed(word)
}

/// Build `WordContent` items from a `word_body` CST node.
fn build_word_contents(
    body_node: Node,
    source: &str,
    errors: &impl ErrorSink,
    items: &mut SmallVec<[WordContent; 2]>,
) {
    let mut cursor = body_node.walk();
    for child in body_node.children(&mut cursor) {
        match child.kind() {
            "word_segment" => {
                let text = extract_utf8_text(child, source, errors, "word_segment", "");
                if let Some(wt) = WordText::new(text) {
                    items.push(WordContent::Text(wt));
                }
            }
            "shortening" => {
                // shortening = '(' word_segment ')'
                // Extract the inner word_segment text
                let mut inner_cursor = child.walk();
                for inner in child.children(&mut inner_cursor) {
                    if inner.kind() == "word_segment" {
                        let text =
                            extract_utf8_text(inner, source, errors, "shortening_content", "");
                        if let Some(ws) = WordShortening::new(text) {
                            items.push(WordContent::Shortening(ws));
                        }
                    }
                }
            }
            "stress_marker" => {
                let text = extract_utf8_text(child, source, errors, "stress_marker", "");
                let marker_type = if text.contains('\u{02C8}') {
                    WordStressMarkerType::Primary
                } else {
                    WordStressMarkerType::Secondary
                };
                items.push(WordContent::StressMarker(WordStressMarker {
                    marker_type,
                    span: Some(talkbank_model::Span::from_usize(
                        child.start_byte(),
                        child.end_byte(),
                    )),
                }));
            }
            "lengthening" => {
                let len_text = extract_utf8_text(child, source, errors, "lengthening", "");
                let colon_count = len_text.chars().filter(|&c| c == ':').count() as u8;
                items.push(WordContent::Lengthening(WordLengthening {
                    count: colon_count.max(1),
                    span: Some(talkbank_model::Span::from_usize(
                        child.start_byte(),
                        child.end_byte(),
                    )),
                }));
            }
            "+" => {
                // Anonymous compound marker
                items.push(WordContent::CompoundMarker(WordCompoundMarker {
                    span: Some(talkbank_model::Span::from_usize(
                        child.start_byte(),
                        child.end_byte(),
                    )),
                }));
            }
            "overlap_point" => {
                // Overlap marker inside word (e.g., butt⌈er⌉)
                // Parse the marker character and optional index digit
                let text = extract_utf8_text(child, source, errors, "overlap_point", "");
                let mut chars = text.chars();
                let kind = match chars.next() {
                    Some('⌈') => OverlapPointKind::TopOverlapBegin,
                    Some('⌉') => OverlapPointKind::TopOverlapEnd,
                    Some('⌊') => OverlapPointKind::BottomOverlapBegin,
                    Some('⌋') => OverlapPointKind::BottomOverlapEnd,
                    _ => OverlapPointKind::TopOverlapBegin, // fallback
                };
                let index = chars
                    .next()
                    .and_then(|c| c.to_digit(10).map(OverlapIndex::new));
                items.push(WordContent::OverlapPoint(OverlapPoint {
                    kind,
                    index,
                    span: Some(talkbank_model::Span::from_usize(
                        child.start_byte(),
                        child.end_byte(),
                    )),
                }));
            }
            "ca_element" => {
                use talkbank_model::content::word::ca::{CAElement, CAElementType};
                let text = extract_utf8_text(child, source, errors, "ca_element", "");
                let span = talkbank_model::Span::from_usize(child.start_byte(), child.end_byte());
                let et = match text.chars().next() {
                    Some('↑') => Some(CAElementType::PitchUp),
                    Some('↓') => Some(CAElementType::PitchDown),
                    Some('↻') => Some(CAElementType::PitchReset),
                    Some('≠') => Some(CAElementType::BlockedSegments),
                    Some('∾') => Some(CAElementType::Constriction),
                    Some('⁑') => Some(CAElementType::Hardening),
                    Some('⤇') => Some(CAElementType::HurriedStart),
                    Some('∙') => Some(CAElementType::Inhalation),
                    Some('Ἡ') => Some(CAElementType::LaughInWord),
                    Some('⤆') => Some(CAElementType::SuddenStop),
                    _ => None,
                };
                if let Some(element_type) = et {
                    items.push(WordContent::CAElement(
                        CAElement::new(element_type).with_span(span),
                    ));
                }
            }
            "ca_delimiter" => {
                use talkbank_model::content::word::ca::{CADelimiter, CADelimiterType};
                let text = extract_utf8_text(child, source, errors, "ca_delimiter", "");
                let span = talkbank_model::Span::from_usize(child.start_byte(), child.end_byte());
                let dt = match text.chars().next() {
                    Some('∆') => Some(CADelimiterType::Faster),
                    Some('∇') => Some(CADelimiterType::Slower),
                    Some('°') => Some(CADelimiterType::Softer),
                    Some('▁') => Some(CADelimiterType::LowPitch),
                    Some('▔') => Some(CADelimiterType::HighPitch),
                    Some('☺') => Some(CADelimiterType::SmileVoice),
                    Some('♋') => Some(CADelimiterType::BreathyVoice),
                    Some('⁇') => Some(CADelimiterType::Unsure),
                    Some('∬') => Some(CADelimiterType::Whisper),
                    Some('Ϋ') => Some(CADelimiterType::Yawn),
                    Some('∮') => Some(CADelimiterType::Singing),
                    Some('↫') => Some(CADelimiterType::SegmentRepetition),
                    Some('⁎') => Some(CADelimiterType::Creaky),
                    Some('◉') => Some(CADelimiterType::Louder),
                    Some('§') => Some(CADelimiterType::Precise),
                    _ => None,
                };
                if let Some(delim_type) = dt {
                    items.push(WordContent::CADelimiter(
                        CADelimiter::new(delim_type).with_span(span),
                    ));
                }
            }
            "underline_begin" => {
                items.push(WordContent::UnderlineBegin(WordUnderlineBegin {
                    span: talkbank_model::Span::from_usize(child.start_byte(), child.end_byte()),
                }));
            }
            "underline_end" => {
                items.push(WordContent::UnderlineEnd(WordUnderlineEnd {
                    span: talkbank_model::Span::from_usize(child.start_byte(), child.end_byte()),
                }));
            }
            "syllable_pause" => {
                items.push(WordContent::SyllablePause(
                    WordSyllablePause::new().with_span(talkbank_model::Span::from_usize(
                        child.start_byte(),
                        child.end_byte(),
                    )),
                ));
            }
            "tilde" => {
                items.push(WordContent::CliticBoundary(
                    WordCliticBoundary::new().with_span(talkbank_model::Span::from_usize(
                        child.start_byte(),
                        child.end_byte(),
                    )),
                ));
            }
            _ => {
                // Skip whitespace and unknown children
            }
        }
    }
}

/// Build a `WordLanguageMarker` from a `word_lang_suffix` token text.
///
/// The token is a single `token.immediate` matching `@s(?::[a-z]{2,3}(?:[+&][a-z]{2,3})*)?`.
/// Examples: `@s`, `@s:eng`, `@s:eng+zho+fra`, `@s:eng&zho&fra`.
fn build_lang_marker(node: Node, source: &str, errors: &impl ErrorSink) -> WordLanguageMarker {
    let text = extract_utf8_text(node, source, errors, "word_lang_suffix", "");

    // Strip the @s prefix
    let after_at_s = text.strip_prefix("@s").unwrap_or("");

    // No colon → bare @s shortcut
    let Some(codes_str) = after_at_s.strip_prefix(':') else {
        return WordLanguageMarker::Shortcut;
    };

    // Check for & separator (ambiguous) vs + separator (multiple)
    if codes_str.contains('&') {
        let codes: Vec<LanguageCode> = codes_str.split('&').map(LanguageCode::new).collect();
        WordLanguageMarker::Ambiguous(codes)
    } else if codes_str.contains('+') {
        let codes: Vec<LanguageCode> = codes_str.split('+').map(LanguageCode::new).collect();
        WordLanguageMarker::Multiple(codes)
    } else {
        // Single code
        WordLanguageMarker::Explicit(LanguageCode::new(codes_str))
    }
}
