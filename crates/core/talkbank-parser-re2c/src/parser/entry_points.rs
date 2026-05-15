//! Public entry point functions — each lexes input and delegates to chumsky parsers.
//!
//! These are the public API that `chat_parser_impl.rs` and tests call.
//! Each function: lex → leaked token slice → chumsky parser → AST.
//!
//! **Memory:** Entry points use `lex_to_tokens` which leaks the NUL-padded
//! source and token slice via `Box::leak`. This is acceptable for
//! small-batch use. For large corpus runs (>5k files), callers should
//! periodically fork a subprocess or accept the memory cost.
//!
//! `parse_chat_file_to_model` provides an owned-result entry point that
//! still leaks internally but is the intended API for batch processing.

use crate::ast::*;
use crate::token::Token;
use talkbank_model::ErrorSink;

use super::{dependent_tiers, file, headers, lex_to_tokens, main_tier};

// ═══════════════════════════════════════════════════════════════
// Owned-result entry point (for batch processing / ChatParser trait)
// ═══════════════════════════════════════════════════════════════

/// Parse a complete CHAT file to an owned model.
///
/// Lex → parse → convert. The intermediate AST borrows from leaked data;
/// the returned model is fully owned (all `String`s, no borrows).
pub fn parse_chat_file_to_model(
    input: &str,
    errors: &impl ErrorSink,
) -> talkbank_model::model::ChatFile {
    let ast = parse_chat_file_streaming(input, errors);
    talkbank_model::model::ChatFile::from(&ast)
}

// ═══════════════════════════════════════════════════════════════
// AST-returning entry points (for tests and direct AST inspection)
// ═══════════════════════════════════════════════════════════════

/// Parse a main tier string starting with '*'.
pub fn parse_main_tier(input: &str) -> Option<MainTier<'_>> {
    use chumsky::Parser as _;
    let tokens = lex_to_tokens(input, 0);
    main_tier::main_tier_parser()
        .parse(tokens)
        .into_result()
        .ok()
}

/// Parse an @ID header content (after `@ID:\t`).
pub fn parse_id_header(input: &str) -> Option<IdHeaderParsed<'_>> {
    use chumsky::Parser as _;
    let tokens = lex_to_tokens(input, crate::lexer::COND_ID_CONTENT);
    headers::id_header_parser().parse(tokens).into_result().ok()
}

/// Parse a @Languages header content (after `@Languages:\t`).
pub fn parse_languages_header(input: &str) -> LanguagesHeaderParsed<'_> {
    use chumsky::Parser as _;
    let tokens = lex_to_tokens(input, crate::lexer::COND_LANGUAGES_CONTENT);
    headers::languages_header_parser()
        .parse(tokens)
        .into_result()
        .unwrap_or_else(|_| LanguagesHeaderParsed { codes: Vec::new() })
}

/// Parse a @Participants header content (after `@Participants:\t`).
pub fn parse_participants_header(input: &str) -> ParticipantsHeaderParsed<'_> {
    use chumsky::Parser as _;
    let tokens = lex_to_tokens(input, crate::lexer::COND_PARTICIPANTS_CONTENT);
    headers::participants_header_parser()
        .parse(tokens)
        .into_result()
        .unwrap_or_else(|_| ParticipantsHeaderParsed {
            entries: Vec::new(),
        })
}

/// Parse a single word (content item) from main tier content.
pub fn parse_word(input: &str) -> Option<WordWithAnnotations<'_>> {
    use chumsky::Parser as _;
    let tokens = lex_to_tokens(input, crate::lexer::COND_MAIN_CONTENT);
    let word_parser =
        chumsky::primitive::choice((main_tier::rich_word(), main_tier::subtoken_word()));
    let item = word_parser.parse(tokens).into_result().ok()?;
    match item {
        ContentItem::Word(w) => Some(w),
        _ => None,
    }
}

/// Parse a single MorWord from %mor content.
pub fn parse_mor_word(input: &str) -> Option<MorWordParsed<'_>> {
    use chumsky::Parser as _;
    let tokens = lex_to_tokens(input, crate::lexer::COND_MOR_CONTENT);
    dependent_tiers::mor_word_parser()
        .parse(tokens)
        .into_result()
        .ok()
}

/// Parse a single GraRelation from %gra content.
pub fn parse_gra_relation(input: &str) -> Option<GraRelationParsed<'_>> {
    let tokens = lex_to_tokens(input, crate::lexer::COND_GRA_CONTENT);
    if let Some(Token::GraRelation {
        index,
        head,
        relation,
    }) = tokens.first().cloned()
    {
        Some(GraRelationParsed {
            index,
            head,
            relation,
        })
    } else {
        None
    }
}

/// Parse a %pho tier body (content after `%pho:\t`).
pub fn parse_pho_tier(input: &str) -> PhoTier<'_> {
    use chumsky::Parser as _;
    let tokens = lex_to_tokens(input, crate::lexer::COND_PHO_CONTENT);
    dependent_tiers::pho_tier_parser()
        .parse(tokens)
        .into_result()
        .unwrap_or_else(|_| PhoTier {
            items: Vec::new(),
            terminator: None,
        })
}

/// Parse a text tier body (content after `%com:\t`, `%act:\t`, etc.).
pub fn parse_text_tier(input: &str) -> TextTierParsed<'_> {
    use chumsky::Parser as _;
    let tokens = lex_to_tokens(input, crate::lexer::COND_TIER_CONTENT);
    dependent_tiers::text_tier_parser()
        .parse(tokens)
        .into_result()
        .unwrap_or_else(|_| TextTierParsed {
            segments: Vec::new(),
        })
}

/// Parse a complete CHAT file (AST, borrows from leaked data).
pub fn parse_chat_file(input: &str) -> ChatFile<'_> {
    let (tokens, source) = super::lex_to_tokens_and_source(input, 0);
    file::parse_file(tokens, source)
}

/// Parse a complete CHAT file with streaming error reporting (AST, borrows).
pub fn parse_chat_file_streaming<'a>(input: &'a str, errors: &impl ErrorSink) -> ChatFile<'a> {
    let (tokens, source) = super::lex_to_tokens_and_source(input, 0);
    file::parse_file_with_errors(tokens, source, errors)
}

/// Parse a %mor tier body.
pub fn parse_mor_tier(input: &str) -> MorTier<'_> {
    use chumsky::Parser as _;
    let tokens = lex_to_tokens(input, crate::lexer::COND_MOR_CONTENT);
    dependent_tiers::mor_tier_parser()
        .parse(tokens)
        .into_result()
        .unwrap_or_else(|_| MorTier {
            items: Vec::new(),
            terminator: None,
        })
}

/// Parse a %gra tier body.
pub fn parse_gra_tier(input: &str) -> GraTier<'_> {
    use chumsky::Parser as _;
    let tokens = lex_to_tokens(input, crate::lexer::COND_GRA_CONTENT);
    dependent_tiers::gra_tier_parser()
        .parse(tokens)
        .into_result()
        .unwrap_or_else(|_| GraTier {
            relations: Vec::new(),
        })
}
