//! Chumsky parser combinators for main tier content parsing.
//!
//! This is the critical module: it replaces the hand-written `parse_contents_until`,
//! `parse_rich_word_with_annotations`, `parse_group`, `parse_quotation`, and related
//! functions with declarative chumsky combinators.
//!
//! The contents parser is recursive (groups contain contents which contain groups).

use chumsky::prelude::*;

use crate::ast::*;
use crate::token::{Token, TokenDiscriminants};

use super::classify::{
    is_annotation, is_linker, is_separator, is_terminator, is_word_token,
    token_to_parsed_annotation, word_to_content_item,
};
use super::dependent_tiers::{opt_newline, ws};
use super::word_body::parse_word_body;

/// Chumsky input type.
type Tokens<'a> = &'a [Token<'a>];

/// Produce the display form of a token, preserving structural delimiters.
///
/// `Token::text()` strips delimiters (e.g., `Shortening("x")` → `"x"`).
/// This function restores them: `Shortening("x")` → `"(x)"`,
/// `Lengthening(":")` → `":"`, prefix tokens → `"&-"`, `"0"`, etc.
fn display_text(tok: &Token<'_>) -> String {
    match tok {
        Token::Shortening(s) => format!("({s})"),
        Token::Zero(_) => "0".to_string(),
        Token::PrefixFiller(_) => "&-".to_string(),
        Token::PrefixNonword(_) => "&~".to_string(),
        Token::PrefixFragment(_) => "&+".to_string(),
        Token::FormMarker(s) => format!("@{s}"),
        Token::WordLangSuffix(None) => "@s".to_string(),
        Token::WordLangSuffix(Some(s)) => format!("@s:{s}"),
        Token::PosTag(s) => format!("${s}"),
        _ => tok.text().to_string(),
    }
}

// ═══════════════════════════════════════════════════════════
// Annotation parsing — shared combinator replacing 4 duplicate loops
// ═══════════════════════════════════════════════════════════

/// Parse a single annotation token into `ParsedAnnotation`.
fn annotation<'a>() -> impl Parser<'a, Tokens<'a>, ParsedAnnotation<'a>> + Clone {
    select! {
        tok if is_annotation(Some(TokenDiscriminants::from(&tok))) => token_to_parsed_annotation(tok),
    }
}

/// Parse trailing annotations: optional whitespace then annotations.
/// Replaces the 4 duplicate save-pos/skip-ws/check-annotation loops.
fn trailing_annotations<'a>() -> impl Parser<'a, Tokens<'a>, Vec<ParsedAnnotation<'a>>> + Clone {
    ws().ignore_then(annotation())
        .repeated()
        .collect::<Vec<_>>()
}

// ═══════════════════════════════════════════════════════════
// Simple content items
// ═══════════════════════════════════════════════════════════

/// Parse a pause token.
fn pause<'a>() -> impl Parser<'a, Tokens<'a>, ContentItem<'a>> + Clone {
    select! {
        tok @ Token::PauseLong(_) => ContentItem::Pause(tok),
        tok @ Token::PauseMedium(_) => ContentItem::Pause(tok),
        tok @ Token::PauseShort(_) => ContentItem::Pause(tok),
        tok @ Token::PauseTimed(_) => ContentItem::Pause(tok),
    }
}

/// Parse a separator token.
fn separator<'a>() -> impl Parser<'a, Tokens<'a>, ContentItem<'a>> + Clone {
    select! {
        tok if is_separator(Some(TokenDiscriminants::from(&tok))) => ContentItem::Separator(tok),
    }
}

/// Parse an overlap point token.
fn overlap_point<'a>() -> impl Parser<'a, Tokens<'a>, ContentItem<'a>> + Clone {
    select! {
        tok @ Token::OverlapTopBegin(_) => ContentItem::OverlapPoint(tok),
        tok @ Token::OverlapTopEnd(_) => ContentItem::OverlapPoint(tok),
        tok @ Token::OverlapBottomBegin(_) => ContentItem::OverlapPoint(tok),
        tok @ Token::OverlapBottomEnd(_) => ContentItem::OverlapPoint(tok),
    }
}

/// Parse structural marker tokens (underlines, long features, nonvocals).
fn structural_markers<'a>() -> impl Parser<'a, Tokens<'a>, ContentItem<'a>> + Clone {
    select! {
        tok @ Token::UnderlineBegin(_) => ContentItem::UnderlineBegin(tok),
        tok @ Token::UnderlineEnd(_) => ContentItem::UnderlineEnd(tok),
        tok @ Token::LongFeatureBegin(_) => ContentItem::LongFeatureBegin(tok),
        tok @ Token::LongFeatureEnd(_) => ContentItem::LongFeatureEnd(tok),
        tok @ Token::NonvocalBegin(_) => ContentItem::NonvocalBegin(tok),
        tok @ Token::NonvocalEnd(_) => ContentItem::NonvocalEnd(tok),
        tok @ Token::NonvocalSimple(_) => ContentItem::NonvocalSimple(tok),
    }
}

/// Parse a freecode annotation.
fn freecode<'a>() -> impl Parser<'a, Tokens<'a>, ContentItem<'a>> + Clone {
    select! { tok @ Token::Freecode(_) => ContentItem::Annotation(tok) }
}

/// Parse an other-spoken-event: &*SPK:word
fn other_spoken_event<'a>() -> impl Parser<'a, Tokens<'a>, ContentItem<'a>> + Clone {
    select! { tok @ Token::OtherSpokenEvent { .. } => ContentItem::OtherSpokenEvent(tok) }
}

/// Parse inline media bullet (within content, not utterance-end).
fn inline_media_bullet<'a>() -> impl Parser<'a, Tokens<'a>, ContentItem<'a>> + Clone {
    select! { tok @ Token::MediaBullet { .. } => ContentItem::MediaBullet(tok) }
}

/// Parse a bare annotation token as content.
fn bare_annotation<'a>() -> impl Parser<'a, Tokens<'a>, ContentItem<'a>> + Clone {
    select! {
        tok if is_annotation(Some(TokenDiscriminants::from(&tok))) => ContentItem::Annotation(tok),
    }
}

// ═══════════════════════════════════════════════════════════
// Rich word parser (Token::Word from lexer)
// ═══════════════════════════════════════════════════════════

/// Parse a rich `Token::Word` into a `ContentItem` (Word or Retrace).
pub fn rich_word<'a>() -> impl Parser<'a, Tokens<'a>, ContentItem<'a>> + Clone {
    select! {
        Token::Word { raw_text, prefix, body, form_marker, lang_suffix, pos_tag } =>
            (raw_text, prefix, body, form_marker, lang_suffix, pos_tag),
    }
    .then(trailing_annotations())
    .map(
        |((raw_text, prefix, body_str, form_marker, lang_suffix_opt, pos_tag), annotations)| {
            let category = match prefix {
                Some("&-") => Some(WordCategory::Filler),
                Some("&~") => Some(WordCategory::Nonword),
                Some("&+") => Some(WordCategory::Fragment),
                Some("0") => Some(WordCategory::Omission),
                _ => None,
            };
            let body = parse_word_body(body_str);
            let lang = match lang_suffix_opt {
                None => None,
                Some("") => Some(ParsedLangSuffix::Shortcut),
                Some(codes) => Some(ParsedLangSuffix::Explicit(codes)),
            };
            let word = WordWithAnnotations {
                category,
                body,
                form_marker,
                lang,
                pos_tag,
                annotations,
                raw_text,
            };
            word_to_content_item(word)
        },
    )
}

// ═══════════════════════════════════════════════════════════
// Legacy word parser (sub-token word assembly)
// ═══════════════════════════════════════════════════════════

/// Parse a word from individual sub-tokens (WordSegment, prefixes, CA markers, etc.).
/// Used when the lexer emits sub-tokens instead of a single rich Word token.
/// This path fires for inputs where the rich Word regex (`w_body`) doesn't match.
pub fn subtoken_word<'a>() -> impl Parser<'a, Tokens<'a>, ContentItem<'a>> + Clone {
    // A word token: any token that is_word_token accepts
    let word_tok = select! {
        tok if is_word_token(TokenDiscriminants::from(&tok)) => tok,
    };

    word_tok
        .repeated()
        .at_least(1)
        .collect::<Vec<Token<'a>>>()
        .then(trailing_annotations())
        .map(|(toks, annotations)| {
            let mut category = None;
            let mut body = Vec::new();
            let mut form_marker = None;
            let mut lang = None;
            let mut pos_tag = None;
            let mut zero_tok = None;

            // Reconstruct raw_text by concatenating display forms of all tokens.
            // Token::text() strips delimiters (e.g., Shortening("x") → "x"),
            // so we use display_text() which preserves them ("(x)").
            let raw_text_owned: String = toks.iter().map(display_text).collect();
            let raw_text: &str = Box::leak(raw_text_owned.into_boxed_str());

            for tok in toks {
                match tok {
                    Token::Zero(_) => {
                        zero_tok = Some(tok.clone());
                        category = Some(WordCategory::Omission);
                    }
                    Token::PrefixFiller(_) => category = Some(WordCategory::Filler),
                    Token::PrefixNonword(_) => category = Some(WordCategory::Nonword),
                    Token::PrefixFragment(_) => category = Some(WordCategory::Fragment),
                    Token::FormMarker(s) => form_marker = Some(s),
                    Token::WordLangSuffix(opt) => {
                        lang = Some(match opt {
                            None => ParsedLangSuffix::Shortcut,
                            Some(codes) => ParsedLangSuffix::Explicit(codes),
                        });
                    }
                    Token::PosTag(s) => pos_tag = Some(s),
                    Token::WordSegment(s) => body.push(WordBodyItem::Text(s)),
                    Token::Shortening(s) => body.push(WordBodyItem::Shortening(s)),
                    Token::Lengthening(s) => body.push(WordBodyItem::Lengthening(s.len() as u8)),
                    Token::CompoundMarker(_) => body.push(WordBodyItem::CompoundMarker),
                    Token::StressPrimary(_) => body.push(WordBodyItem::Stress(StressKind::Primary)),
                    Token::StressSecondary(_) => {
                        body.push(WordBodyItem::Stress(StressKind::Secondary))
                    }
                    Token::OverlapTopBegin(s) => {
                        body.push(WordBodyItem::OverlapPoint(OverlapKind::TopBegin, s))
                    }
                    Token::OverlapTopEnd(s) => {
                        body.push(WordBodyItem::OverlapPoint(OverlapKind::TopEnd, s))
                    }
                    Token::OverlapBottomBegin(s) => {
                        body.push(WordBodyItem::OverlapPoint(OverlapKind::BottomBegin, s))
                    }
                    Token::OverlapBottomEnd(s) => {
                        body.push(WordBodyItem::OverlapPoint(OverlapKind::BottomEnd, s))
                    }
                    Token::SyllablePause(_) => body.push(WordBodyItem::SyllablePause),
                    Token::Tilde(_) => body.push(WordBodyItem::CliticBoundary),
                    Token::CaBlockedSegments(_) => {
                        body.push(WordBodyItem::CaElement(CaElementKind::BlockedSegments))
                    }
                    Token::CaConstriction(_) => {
                        body.push(WordBodyItem::CaElement(CaElementKind::Constriction))
                    }
                    Token::CaHardening(_) => {
                        body.push(WordBodyItem::CaElement(CaElementKind::Hardening))
                    }
                    Token::CaHurriedStart(_) => {
                        body.push(WordBodyItem::CaElement(CaElementKind::HurriedStart))
                    }
                    Token::CaInhalation(_) => {
                        body.push(WordBodyItem::CaElement(CaElementKind::Inhalation))
                    }
                    Token::CaLaughInWord(_) => {
                        body.push(WordBodyItem::CaElement(CaElementKind::LaughInWord))
                    }
                    Token::CaPitchDown(_) => {
                        body.push(WordBodyItem::CaElement(CaElementKind::PitchDown))
                    }
                    Token::CaPitchReset(_) => {
                        body.push(WordBodyItem::CaElement(CaElementKind::PitchReset))
                    }
                    Token::CaPitchUp(_) => {
                        body.push(WordBodyItem::CaElement(CaElementKind::PitchUp))
                    }
                    Token::CaSuddenStop(_) => {
                        body.push(WordBodyItem::CaElement(CaElementKind::SuddenStop))
                    }
                    Token::CaUnsure(_) => {
                        body.push(WordBodyItem::CaDelimiter(CaDelimiterKind::Unsure))
                    }
                    Token::CaPrecise(_) => {
                        body.push(WordBodyItem::CaDelimiter(CaDelimiterKind::Precise))
                    }
                    Token::CaCreaky(_) => {
                        body.push(WordBodyItem::CaDelimiter(CaDelimiterKind::Creaky))
                    }
                    Token::CaSofter(_) => {
                        body.push(WordBodyItem::CaDelimiter(CaDelimiterKind::Softer))
                    }
                    Token::CaSegmentRepetition(_) => body.push(WordBodyItem::CaDelimiter(
                        CaDelimiterKind::SegmentRepetition,
                    )),
                    Token::CaFaster(_) => {
                        body.push(WordBodyItem::CaDelimiter(CaDelimiterKind::Faster))
                    }
                    Token::CaSlower(_) => {
                        body.push(WordBodyItem::CaDelimiter(CaDelimiterKind::Slower))
                    }
                    Token::CaWhisper(_) => {
                        body.push(WordBodyItem::CaDelimiter(CaDelimiterKind::Whisper))
                    }
                    Token::CaSinging(_) => {
                        body.push(WordBodyItem::CaDelimiter(CaDelimiterKind::Singing))
                    }
                    Token::CaLowPitch(_) => {
                        body.push(WordBodyItem::CaDelimiter(CaDelimiterKind::LowPitch))
                    }
                    Token::CaHighPitch(_) => {
                        body.push(WordBodyItem::CaDelimiter(CaDelimiterKind::HighPitch))
                    }
                    Token::CaLouder(_) => {
                        body.push(WordBodyItem::CaDelimiter(CaDelimiterKind::Louder))
                    }
                    Token::CaSmileVoice(_) => {
                        body.push(WordBodyItem::CaDelimiter(CaDelimiterKind::SmileVoice))
                    }
                    Token::CaBreathyVoice(_) => {
                        body.push(WordBodyItem::CaDelimiter(CaDelimiterKind::BreathyVoice))
                    }
                    Token::CaYawn(_) => body.push(WordBodyItem::CaDelimiter(CaDelimiterKind::Yawn)),
                    Token::Ampersand(_) => body.push(WordBodyItem::Text("&")),
                    other => body.push(WordBodyItem::Text(other.text())),
                }
            }

            // Standalone zero (0) with no body → Action, not Word.
            // This distinguishes `0` (action) from `0word` (omission).
            if category == Some(WordCategory::Omission)
                && body.is_empty()
                && let Some(zt) = zero_tok
            {
                return ContentItem::Action {
                    zero: zt,
                    annotations,
                };
            }

            let word = WordWithAnnotations {
                category,
                body,
                form_marker,
                lang,
                pos_tag,
                annotations,
                raw_text,
            };
            word_to_content_item(word)
        })
}

// ═══════════════════════════════════════════════════════════
// Event parser
// ═══════════════════════════════════════════════════════════

/// Parse an event with optional trailing annotations.
/// Retrace markers on events are silently dropped (not semantically valid).
fn event<'a>() -> impl Parser<'a, Tokens<'a>, ContentItem<'a>> + Clone {
    select! { tok @ Token::Event(_) => tok }
        .then(trailing_annotations())
        .map(|(event_tok, annotations)| {
            // Drop retrace annotations on events
            let annotations: Vec<_> = annotations
                .into_iter()
                .filter(|a| !a.is_retrace())
                .collect();
            if annotations.is_empty() {
                ContentItem::Event(vec![event_tok])
            } else {
                ContentItem::AnnotatedEvent {
                    event: event_tok,
                    annotations,
                }
            }
        })
}

// ═══════════════════════════════════════════════════════════
// Contents parser (recursive)
// ═══════════════════════════════════════════════════════════

/// Build the recursive contents parser.
///
/// Returns a parser that produces `Vec<ContentItem>` — the content items
/// of a tier body (main tier or inside groups/quotations).
pub fn contents_parser<'a>() -> impl Parser<'a, Tokens<'a>, Vec<ContentItem<'a>>> + Clone {
    recursive(|contents| {
        // Group: < contents > annotations
        let group = select! { Token::LessThan(_) => () }
            .ignore_then(contents.clone())
            .then_ignore(select! { Token::GreaterThan(_) => () })
            .then(trailing_annotations())
            .map(|(contents, annotations)| {
                // Check for retrace annotation — if found, this is a retrace group
                let retrace_idx = annotations.iter().position(|a| a.is_retrace());
                if let Some(idx) = retrace_idx {
                    let mut annotations = annotations;
                    let ann = annotations.remove(idx);
                    // `is_retrace()` guard above; `retrace_kind()`
                    // returns `Some` for is_retrace annotations.
                    #[allow(clippy::expect_used)]
                    let kind = ann.retrace_kind().expect("is_retrace was true");
                    ContentItem::Retrace(Retrace {
                        content: contents,
                        kind,
                        is_group: true,
                        annotations,
                    })
                } else if annotations.is_empty() {
                    // grammar.js: `group_with_annotations` REQUIRES at
                    // least one annotation. Tree-sitter recovers from a
                    // missing annotation by inserting a synthetic MISSING
                    // `retrace_complete`, producing
                    // `Retrace { kind: Full, is_group: true }` in the model.
                    // Mirror that recovery here so SemanticEq holds on
                    // malformed input. The diagnostic emission for this
                    // recovery path is tracked separately — the chumsky
                    // combinator does not currently have ErrorSink access;
                    // see `docs/parity-report.md` § MISSING-Token Recovery
                    // Policy.
                    ContentItem::Retrace(Retrace {
                        content: contents,
                        kind: RetraceKindParsed::Complete,
                        is_group: true,
                        annotations,
                    })
                } else {
                    ContentItem::Group(Group {
                        contents,
                        annotations,
                    })
                }
            });

        // Quotation: " contents "
        let quotation = select! { Token::LeftDoubleQuote(_) => () }
            .ignore_then(contents.clone())
            .then_ignore(select! { Token::RightDoubleQuote(_) => () })
            .map(|contents| ContentItem::Quotation(Quotation { contents }));

        // PhoGroup: ‹ contents ›
        let pho_group = select! { Token::PhoGroupBegin(_) => () }
            .ignore_then(contents.clone())
            .then_ignore(select! { Token::PhoGroupEnd(_) => () })
            .map(ContentItem::PhoGroup);

        // SinGroup: 〔 contents 〕
        let sin_group = select! { Token::SinGroupBegin(_) => () }
            .ignore_then(contents)
            .then_ignore(select! { Token::SinGroupEnd(_) => () })
            .map(ContentItem::SinGroup);

        // A single content item: one of many alternatives.
        // Order matters for chumsky's `choice` — more specific alternatives first.
        // overlap_point and pause must precede subtoken_word because their tokens
        // are also in is_word_token (overlap markers, etc.).
        let content_item = choice((
            group,
            quotation,
            pho_group,
            sin_group,
            event(),
            other_spoken_event(),
            rich_word(),
            overlap_point(),
            pause(),
            separator(),
            subtoken_word(),
            freecode(),
            structural_markers(),
            inline_media_bullet(),
            bare_annotation(),
        ));

        // Contents: one or more content items separated by whitespace
        content_item.padded_by(ws()).repeated().collect::<Vec<_>>()
    })
}

// ═══════════════════════════════════════════════════════════
// Terminator
// ═══════════════════════════════════════════════════════════

/// Parse a terminator token.
fn terminator<'a>() -> impl Parser<'a, Tokens<'a>, Token<'a>> + Clone {
    select! {
        tok if is_terminator(Some(TokenDiscriminants::from(&tok))) => tok,
    }
}

/// Parse postcodes: [+ code] tokens.
fn postcodes<'a>() -> impl Parser<'a, Tokens<'a>, Vec<Token<'a>>> + Clone {
    select! { tok @ Token::Postcode(_) => tok }
        .padded_by(ws())
        .repeated()
        .collect::<Vec<_>>()
}

// ═══════════════════════════════════════════════════════════
// Tier body and main tier
// ═══════════════════════════════════════════════════════════

/// Parse linkers: repeat1(linker whitespace*)
fn linkers<'a>() -> impl Parser<'a, Tokens<'a>, Vec<Token<'a>>> + Clone {
    select! {
        tok if is_linker(Some(TokenDiscriminants::from(&tok))) => tok,
    }
    .then_ignore(ws())
    .repeated()
    .collect::<Vec<_>>()
}

/// Parse a complete tier body.
///
/// grammar.js: tier_body = seq(
///   optional(linkers),
///   optional(seq(langcode, whitespaces)),
///   contents,
///   utterance_end
/// )
pub fn tier_body_parser<'a>() -> impl Parser<'a, Tokens<'a>, TierBody<'a>> + Clone {
    let langcode = select! { tok @ Token::Langcode(_) => tok }.then_ignore(ws());

    // Utterance end: optional(terminator), optional(postcodes), optional(media_bullet), newline
    let media_bullet = select! { tok @ Token::MediaBullet { .. } => tok };

    linkers()
        .then(langcode.or_not())
        .then(contents_parser())
        .then(ws().ignore_then(terminator()).or_not())
        .then(postcodes())
        .then(ws().ignore_then(media_bullet).or_not())
        .then_ignore(ws())
        .then_ignore(opt_newline())
        .map(
            |(((((linkers, langcode), contents), terminator_tok), postcodes), media_bullet)| {
                TierBody {
                    linkers,
                    langcode,
                    contents,
                    terminator: terminator_tok,
                    postcodes,
                    media_bullet,
                }
            },
        )
}

/// Parse a complete main tier line.
///
/// grammar.js: main_tier = seq(star, speaker, colon, tab, tier_body)
pub fn main_tier_parser<'a>() -> impl Parser<'a, Tokens<'a>, MainTier<'a>> + Clone {
    let star = select! { Token::Star(_) => () };
    let speaker = select! { tok @ Token::Speaker(_) => tok };
    let tier_sep = select! { Token::TierSep(_) => () };

    star.ignore_then(speaker)
        .then_ignore(tier_sep)
        .then(tier_body_parser())
        .map(|(speaker, tier_body)| MainTier { speaker, tier_body })
}
