//! Chumsky parser combinators for header parsing.
//!
//! Header parsers: @ID, @Languages, @Participants.

use chumsky::prelude::*;

use crate::ast::*;
use crate::token::Token;

use super::dependent_tiers::{opt_newline, ws};

/// Chumsky input type.
type Tokens<'a> = &'a [Token<'a>];

// ═══════════════════════════════════════════════════════════
// @ID header — extract 10 pipe-delimited fields from IdFields token
// ═══════════════════════════════════════════════════════════

/// Parse an `@ID` header content.
pub fn id_header_parser<'a>() -> impl Parser<'a, Tokens<'a>, IdHeaderParsed<'a>> + Clone {
    select! {
        Token::IdFields { language, corpus, speaker, age, sex, group, ses, role, education, custom }
            => IdHeaderParsed { language, corpus, speaker, age, sex, group, ses, role, education, custom_field: custom },
    }
    .then_ignore(ws())
    .then_ignore(opt_newline())
}

// ═══════════════════════════════════════════════════════════
// @Languages header — comma-separated language codes
// ═══════════════════════════════════════════════════════════

/// Parse a `@Languages` header content.
pub fn languages_header_parser<'a>()
-> impl Parser<'a, Tokens<'a>, LanguagesHeaderParsed<'a>> + Clone {
    let code = select! { Token::LanguageCode(s) => s };
    let comma = select! { Token::Comma(_) => () };

    ws().ignore_then(
        code.separated_by(ws().then(comma).then(ws()))
            .allow_trailing()
            .collect::<Vec<_>>(),
    )
    .then_ignore(ws())
    .then_ignore(opt_newline())
    .map(|codes| LanguagesHeaderParsed { codes })
}

// ═══════════════════════════════════════════════════════════
// @Participants header — comma-separated entries (SPK Name Role)
// ═══════════════════════════════════════════════════════════

/// Parse a `@Participants` header content.
pub fn participants_header_parser<'a>()
-> impl Parser<'a, Tokens<'a>, ParticipantsHeaderParsed<'a>> + Clone {
    let word = select! { Token::ParticipantWord(s) => s };
    let comma = select! { Token::Comma(_) => () };

    // A single participant entry: one or more words
    let entry = word
        .separated_by(ws())
        .at_least(1)
        .collect::<Vec<_>>()
        .map(|words| ParticipantEntryParsed { words });

    ws().ignore_then(
        entry
            .separated_by(ws().then(comma).then(ws()))
            .allow_trailing()
            .collect::<Vec<_>>(),
    )
    .then_ignore(ws())
    .then_ignore(opt_newline())
    .map(|entries| ParticipantsHeaderParsed { entries })
}
